#[macro_use]
extern crate num_derive;

use ethercat_soem_ctx as ctx;
use ethercat_types as ec;
use num_traits::cast::FromPrimitive;
use std::{collections::HashMap, convert::TryFrom, ffi::CString, time::Duration};

mod al_status;
mod error;
mod util;

pub use self::{al_status::*, error::Error};

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_micros(2_000);
const DEFAULT_SDO_TIMEOUT: Duration = Duration::from_millis(3_000);

const MAX_SM_CNT: u8 = 8;

const SDO_IDX_PDO_ASSIGN: ec::Idx = ec::Idx::new(0x1C10);
const SDO_IDX_SM_COMM_TYPE: ec::Idx = ec::Idx::new(0x1C00);

const SM_TYPE_OUTPUTS: u8 = 3;
const SM_TYPE_INPUTS: u8 = 4;

const EC_NOFRAME: i32 = -1;

type Result<T> = std::result::Result<T, Error>;

type SdoInfo = Vec<(ec::SdoInfo, Vec<Option<ec::SdoEntryInfo>>)>;
type PdoInfo = Vec<(ec::PdoInfo, Vec<(ec::PdoEntryInfo, ec::SdoIdx)>)>;

pub struct Master {
    ctx: Box<ctx::Ctx>,
    sdos: HashMap<ec::SlavePos, SdoInfo>,
    pdos: HashMap<ec::SlavePos, PdoInfo>,
}

impl Master {
    pub fn try_new<S: Into<String>>(iface: S) -> Result<Self> {
        let mut master = Self {
            ctx: Box::new(ctx::Ctx::default()),
            sdos: HashMap::new(),
            pdos: HashMap::new(),
        };
        master.init(iface.into())?;
        Ok(master)
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn ptr(&mut self) -> *mut ctx::Ctx {
        let reference: &mut ctx::Ctx = &mut *self.ctx;
        reference as *mut ctx::Ctx
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr(ctx_ptr: *mut ctx::Ctx) -> Self {
        Master::from_ptr_with_caches(ctx_ptr, HashMap::new(), HashMap::new())
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr_with_caches(
        ctx_ptr: *mut ctx::Ctx,
        sdos: HashMap<ec::SlavePos, SdoInfo>,
        pdos: HashMap<ec::SlavePos, PdoInfo>,
    ) -> Self {
        Master {
            ctx: Box::from_raw(ctx_ptr),
            sdos,
            pdos,
        }
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn sdo_info_cache(&self) -> &HashMap<ec::SlavePos, SdoInfo> {
        &self.sdos
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn pdo_info_cache(&self) -> &HashMap<ec::SlavePos, PdoInfo> {
        &self.pdos
    }

    fn init(&mut self, iface: String) -> Result<()> {
        log::debug!("Initialise SOEM stack: bind socket to {}", iface);
        let iface = CString::new(iface).map_err(|_| Error::Iface)?;
        let res = self.ctx.init(iface);
        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::Init);
        }
        Ok(())
    }

    /// Automatically configure slaves and fetch SDO & PDO information.
    pub fn auto_config(&mut self) -> Result<()> {
        log::debug!("Find and auto-config slaves");
        self.request_states(ec::AlState::Init)?;
        self.check_states(ec::AlState::Init, Duration::from_millis(500))?;
        let usetable = false;
        let res = self.ctx.config_init(usetable);
        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(match res {
                -1 => Error::NoFrame,
                -2 => Error::OtherFrame,
                _ => Error::NoSlaves,
            });
        }
        let slave_count = self.ctx.slave_count();
        log::debug!("{} slaves found", slave_count);
        self.request_states(ec::AlState::PreOp)?;
        self.check_states(ec::AlState::PreOp, Duration::from_millis(500))?;
        let group = 0;
        let io_map_size = self.ctx.config_map_group(group);
        if io_map_size <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::CfgMapGroup);
        }
        let res = self.ctx.config_dc();
        if res == 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::CfgDc);
        }
        log::debug!("Fetch SDO info");
        for i in 0..slave_count {
            let pos = ec::SlavePos::from(i as u16);
            let sdo_info = self.read_od_list(pos)?;
            self.sdos.insert(pos, sdo_info);
        }
        log::debug!("Fetch PDO info");
        self.pdos = self.coe_pdo_info()?;
        Ok(())
    }

    fn coe_pdo_info(&mut self) -> Result<HashMap<ec::SlavePos, PdoInfo>> {
        let mut res = HashMap::new();

        for slave in 0..self.ctx.slave_count() as u16 {
            let slave_pos = ec::SlavePos::new(slave);

            let obj_cnt = self.sm_comm_type(slave_pos, 0)?;
            if obj_cnt <= 2 {
                log::warn!("Slave {}: found less than two sync manager types", slave);
                continue;
            }

            let mut sm_cnt = obj_cnt - 1; // make sm_cnt equal to number of defined SM

            if sm_cnt > MAX_SM_CNT {
                log::debug!(
                    "Slave {}: limit to max. {} number of sync managers",
                    slave,
                    MAX_SM_CNT
                );
                sm_cnt = MAX_SM_CNT;
            }

            let mut pdo_info = vec![];

            let mut sm_types = vec![];

            for sm in 2..=sm_cnt {
                let sm_type = self.sm_comm_type(slave_pos, sm + 1)?;
                if sm == 2 && sm_type == 2 {
                    log::warn!(
                        "SM2 has type 2 == mailbox out, this is a bug in {:?}!",
                        slave_pos
                    );
                    continue;
                }
                sm_types.push((sm, sm_type));
            }

            let mut pdo_cnt_offset = 0;
            let mut pdo_entry_cnt_offset = 0;

            for t in &[SM_TYPE_OUTPUTS, SM_TYPE_INPUTS] {
                for (sm, sm_type) in sm_types.iter().filter(|(_, sm_type)| sm_type == t) {
                    let pdo_assign =
                        self.si_pdo_assign(slave_pos, *sm, pdo_cnt_offset, pdo_entry_cnt_offset)?;
                    log::debug!(
                        "Slave {}: SM {} (type: {}): read the assigned PDOs",
                        slave,
                        sm,
                        sm_type
                    );
                    pdo_cnt_offset += pdo_assign.len() as u8;
                    pdo_entry_cnt_offset += pdo_assign
                        .iter()
                        .map(|(_, entries)| entries.len())
                        .sum::<usize>() as u8;
                    pdo_info.extend_from_slice(&pdo_assign);
                }
            }
            res.insert(slave_pos, pdo_info);
        }
        Ok(res)
    }

    /// Read PDO assign structure
    fn si_pdo_assign(
        &mut self,
        slave: ec::SlavePos,
        sm: u8,
        pdo_pos_offset: u8,
        pdo_entry_pos_offset: u8,
    ) -> Result<Vec<(ec::PdoInfo, Vec<(ec::PdoEntryInfo, ec::SdoIdx)>)>> {
        let idx = ec::Idx::new(u16::from(SDO_IDX_PDO_ASSIGN) + sm as u16);

        let mut pdo_entry_pos = pdo_entry_pos_offset;

        let pdo_cnt = self.read_sdo_entry(
            slave,
            ec::SdoIdx {
                idx,
                sub_idx: ec::SubIdx::new(0),
            },
            DEFAULT_SDO_TIMEOUT,
        )?;
        let pdo_cnt = match pdo_cnt {
            ec::Value::U8(x) => x,
            _ => {
                log::warn!(
                    "Could not read number of PDOs from {:?}: unexpected data type",
                    slave
                );
                return Err(Error::UnexpectedDataType);
            }
        };
        log::debug!("Available sub indexes of {:?}: {}", idx, pdo_cnt);

        let mut pdos = vec![];

        // read all PDO's
        for pdo_sub in 1..=pdo_cnt {
            let pdo_pos = pdo_pos_offset + pdo_sub - 1;
            log::debug!(
                "{:?}: read PDO IDX from 0x{:X}.0x{:X}",
                slave,
                u16::from(idx),
                pdo_sub
            );
            let val = self.read_sdo_entry(
                slave,
                ec::SdoIdx {
                    idx,
                    sub_idx: pdo_sub.into(),
                },
                DEFAULT_SDO_TIMEOUT,
            )?;

            let pdo_idx = match val {
                ec::Value::U16(idx) => {
                    let idx = u16::from_be(idx);
                    ec::Idx::new(idx)
                }
                _ => {
                    log::warn!(
                        "Could not read number of PDOs from {:?}: unexpected data type",
                        slave
                    );
                    return Err(Error::UnexpectedDataType);
                }
            };
            log::debug!("... PDO IDX is 0x{:X}", u16::from(pdo_idx));

            log::debug!(
                "{:?}: read PDO count from 0x{:X}.0x{:X}",
                slave,
                u16::from(pdo_idx),
                0
            );
            let val = self.read_sdo_entry(
                slave,
                ec::SdoIdx {
                    idx: pdo_idx,
                    sub_idx: ec::SubIdx::new(0),
                },
                DEFAULT_SDO_TIMEOUT,
            )?;
            log::debug!("... PDO count is {:?}", val);

            let pdo_entry_cnt = match val {
                ec::Value::U8(n) => n,
                _ => {
                    log::warn!("Could not read number of PDOs entries from {:?} {:?}: unexpected data type", pdo_idx, slave);
                    return Err(Error::UnexpectedDataType);
                }
            };

            let mut pdo_entries = vec![];

            for entry_sub in 1..=pdo_entry_cnt {
                let pdo_data_sdo_idx = ec::SdoIdx {
                    idx: pdo_idx,
                    sub_idx: ec::SubIdx::new(entry_sub),
                };

                let val = self.read_sdo_entry(slave, pdo_data_sdo_idx, DEFAULT_SDO_TIMEOUT)?;

                let data = match val {
                    ec::Value::U32(x) => u32::from_be(x),
                    _ => {
                        log::warn!(
                            "Could not read PDO entry data from {:?} {:?}: unexpected data type",
                            pdo_data_sdo_idx,
                            slave
                        );
                        return Err(Error::UnexpectedDataType);
                    }
                };
                let bit_len = (data & 0x_00FF) as u8;
                let obj_idx = (data >> 16) as u16;
                let obj_subidx = ((data >> 8) & 0x_0000_00FF) as u8;

                let sdo_idx = ec::SdoIdx::new(obj_idx, obj_subidx);
                let sdo_entry = self.cached_sdo_entry(slave, sdo_idx);

                let name = match sdo_entry {
                    Some(e) => {
                        debug_assert_eq!(bit_len as u16, e.bit_len);
                        e.description.clone()
                    }
                    None => {
                        log::warn!("Could not find SDO ({:?}) entry description", sdo_idx);
                        String::new()
                    }
                };

                let pdo_entry_info = ec::PdoEntryInfo {
                    pos: ec::PdoEntryPos::new(pdo_entry_pos),
                    entry_idx: ec::PdoEntryIdx {
                        idx: obj_idx.into(),
                        sub_idx: ec::SubIdx::new(obj_subidx),
                    },
                    bit_len,
                    name,
                };
                pdo_entries.push((pdo_entry_info, sdo_idx));
                pdo_entry_pos += 1;
            }
            if let Some((e, _)) = pdo_entries.iter().nth(0) {
                let sdo_idx = e.entry_idx.idx;
                let sdo_info = self.cached_sdo_info(slave, sdo_idx);

                let name = match sdo_info {
                    Some(info) => info.name.clone(),
                    None => {
                        log::warn!("Could not find SDO ({:?}) name", sdo_idx);
                        String::new()
                    }
                };

                let pdo_info = ec::PdoInfo {
                    sm: ec::SmIdx::new(sm),
                    pos: ec::PdoPos::new(pdo_pos),
                    idx: pdo_idx,
                    entry_count: pdo_entry_cnt,
                    name,
                };
                pdos.push((pdo_info, pdo_entries));
            }
        }
        Ok(pdos)
    }

    fn cached_sdo_entry(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
    ) -> Option<&ec::SdoEntryInfo> {
        self.sdos
            .get(&slave)
            .and_then(|sdos| sdos.iter().find(|(info, _entries)| info.idx == idx.idx))
            .map(|(_, x)| x)
            .and_then(|entries| entries.get(u8::from(idx.sub_idx) as usize))
            .and_then(Option::as_ref)
    }

    fn cached_sdo_info(&mut self, slave: ec::SlavePos, idx: ec::Idx) -> Option<&ec::SdoInfo> {
        self.sdos
            .get(&slave)
            .and_then(|sdos| sdos.iter().find(|(info, _entries)| info.idx == idx))
            .map(|(x, _)| x)
    }

    fn sm_comm_type(&mut self, slave: ec::SlavePos, sub_idx: u8) -> Result<u8> {
        let val = self.read_sdo_entry(
            slave,
            ec::SdoIdx {
                idx: SDO_IDX_SM_COMM_TYPE,
                sub_idx: sub_idx.into(),
            },
            DEFAULT_SDO_TIMEOUT,
        )?;
        let val = match val {
            ec::Value::U8(x) => x,
            _ => {
                log::warn!("Could not read SyncManager communication type information of {:?}: unexpected data type", slave);
                return Err(Error::UnexpectedDataType);
            }
        };
        Ok(val)
    }

    pub fn request_states(&mut self, state: ec::AlState) -> Result<()> {
        log::debug!("wait for all slaves to reach {:?} state", state);
        let s = u8::from(state) as u16;
        for i in 0..=self.ctx.slave_count() {
            self.ctx.slaves_mut()[i].set_state(s);
        }
        match self.ctx.write_state(0) {
            EC_NOFRAME => Err(Error::NoFrame),
            0 => {
                if self.ctx.is_err() {
                    log::debug!("Context errors: {:?}", self.ctx_errors());
                }
                log::warn!("Could not set state {:?} for slaves", state);
                Err(Error::SetState)
            }
            _ => Ok(()),
        }
    }

    pub fn check_states(&mut self, state: ec::AlState, timeout: Duration) -> Result<()> {
        let res = self.ctx.state_check(0, u8::from(state) as u16, timeout);
        if res == 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            log::warn!("Could not check state {:?} for slaves", state);
            return Err(Error::CheckState);
        }
        let found_state = ec::AlState::try_from(res as u8).map_err(|_| {
            log::warn!("Could not translate u16 `{}` into AlState", res);
            Error::CheckState
        })?;
        if found_state != state {
            log::warn!(
                "Current state {:?} does not match expected state {:?}",
                found_state,
                state
            );
            return Err(Error::CheckState);
        }
        Ok(())
    }

    pub fn slaves(&mut self) -> &mut [ctx::Slave] {
        let cnt = self.ctx.slave_count();
        &mut self.ctx.slaves_mut()[1..=cnt]
    }

    pub fn states(&mut self) -> Result<Vec<ec::AlState>> {
        let lowest_state = self.ctx.read_state();
        if lowest_state <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::ReadStates);
        }
        let states = (1..=self.ctx.slave_count())
            .into_iter()
            .map(|i| self.slave_state(i))
            .collect::<Result<_>>()?;
        Ok(states)
    }

    fn slave_state(&self, slave: usize) -> Result<ec::AlState> {
        let s = &self.ctx.slaves()[slave];
        let state = s.state();
        let status_code = s.al_status_code();
        ec::AlState::try_from(state as u8).map_err(|_| Error::AlState(AlStatus::from(status_code)))
    }

    pub fn send_processdata(&mut self) -> Result<()> {
        self.ctx.send_processdata();
        if self.ctx.is_err() {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::SendProcessData);
        }
        Ok(())
    }

    pub fn recv_processdata(&mut self) -> Result<usize> {
        let wkc = self.ctx.receive_processdata(DEFAULT_RECV_TIMEOUT);
        if self.ctx.is_err() {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::RecvProcessData);
        }
        Ok(wkc as usize)
    }

    pub fn group_outputs_wkc(&mut self, i: usize) -> Result<usize> {
        if i >= self.ctx.groups().len() || i >= self.max_group() {
            if self.ctx.is_err() {
                log::debug!("Context errors: {:?}", self.ctx_errors());
            }
            return Err(Error::GroupId);
        }
        Ok(self.ctx.groups()[i].outputs_wkc() as usize)
    }

    pub fn group_inputs_wkc(&mut self, i: usize) -> Result<usize> {
        if i >= self.ctx.groups().len() || i >= self.max_group() {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::GroupId);
        }
        Ok(self.ctx.groups()[i].inputs_wkc() as usize)
    }

    pub fn slave_count(&self) -> usize {
        self.ctx.slave_count() as usize
    }

    pub fn max_group(&self) -> usize {
        self.ctx.max_group() as usize
    }

    pub fn dc_time(&self) -> i64 {
        self.ctx.dc_time()
    }

    fn read_od_desc(&mut self, item: u16, od_list: &mut ctx::OdList) -> Result<ec::SdoInfo> {
        let res = self.ctx.read_od_description(item, od_list);
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::ReadOdDesc(pos));
        }
        let i = item as usize;
        let idx = ec::Idx::from(od_list.indexes()[i]);
        let object_code = Some(od_list.object_codes()[i]);
        let name = od_list.names()[i].clone();
        let max_sub_idx = ec::SubIdx::from(od_list.max_subs()[i]);
        let info = ec::SdoInfo {
            pos,
            idx,
            max_sub_idx,
            object_code,
            name,
        };
        Ok(info)
    }

    fn read_oe_list(&mut self, item: u16, od_list: &mut ctx::OdList) -> Result<ctx::OeList> {
        let mut oe_list = ctx::OeList::default();
        let res = self.ctx.read_oe(item, od_list, &mut oe_list);
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::ReadOeList(pos));
        }
        Ok(oe_list)
    }

    pub fn read_od_list(&mut self, slave: ec::SlavePos) -> Result<SdoInfo> {
        let mut od_list = ctx::OdList::default();
        let res = self.ctx.read_od_list(u16::from(slave) + 1, &mut od_list);

        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::ReadOdList(slave));
        }
        log::debug!(
            "CoE Object Description: found {} entries",
            od_list.entries()
        );
        let mut sdos = vec![];
        for i in 0..od_list.entries() {
            let sdo_info = self.read_od_desc(i as u16, &mut od_list)?;
            let oe_list = self.read_oe_list(i as u16, &mut od_list)?;
            let mut entries = vec![];
            for j in 0..=u8::from(sdo_info.max_sub_idx) as usize {
                let dt = oe_list.data_types()[j];
                let len = oe_list.bit_lengths()[j];
                let bit_len = if len == 0 { None } else { Some(len) };
                let info = ec::DataType::from_u16(dt)
                    .zip(bit_len)
                    .map(|(data_type, bit_len)| {
                        let access = access_from_u16(oe_list.object_access()[j]);
                        let description = oe_list.names()[j].clone();
                        ec::SdoEntryInfo {
                            data_type,
                            bit_len,
                            access,
                            description,
                        }
                    });
                if info.is_none() {
                    log::warn!(
                        "Invalid entry at {:?} index {}: Unknown data type ({}) with bit length {}",
                        sdo_info.pos,
                        j,
                        dt,
                        len
                    );
                }
                entries.push(info);
            }
            sdos.push((sdo_info, entries));
        }
        Ok(sdos)
    }

    pub fn read_sdo<'t>(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
        access_complete: bool,
        target: &'t mut [u8],
        timeout: Duration,
    ) -> Result<&'t mut [u8]> {
        let index = u16::from(idx.idx);
        let subindex = u8::from(idx.sub_idx);
        let (wkc, slice) = self.ctx.sdo_read(
            u16::from(slave) + 1,
            index,
            subindex,
            access_complete,
            target,
            timeout,
        );
        if wkc <= 0 {
            let errs = self.ctx_errors();
            log::debug!("Context errors: {:?}", errs);
            for e in errs {
                if e.err_type == ctx::ErrType::Packet && e.abort_code == 3 {
                    log::warn!("data container too small for type");
                }
            }
            return Err(Error::ReadSdo(slave, idx));
        }
        Ok(slice)
    }

    pub fn read_sdo_entry(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
        timeout: Duration,
    ) -> Result<ec::Value> {
        let info = self
            .sdos
            .get(&slave)
            .and_then(|info| info.iter().find(|(info, _)| info.idx == idx.idx))
            .and_then(|(_, entries)| entries.get(u8::from(idx.sub_idx) as usize))
            .and_then(Option::as_ref)
            .ok_or(Error::SubIdxNotFound(slave, idx))?;
        let dt = info.data_type;
        let len = info.bit_len as usize;
        let byte_count = if len % 8 == 0 { len / 8 } else { (len / 8) + 1 };
        let mut target = vec![0; byte_count];
        let raw_value = self.read_sdo(slave, idx, false, &mut target, timeout)?;
        debug_assert!(!raw_value.is_empty());
        Ok(util::value_from_slice(dt, raw_value)?)
    }

    pub fn read_sdo_complete(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::Idx,
        timeout: Duration,
    ) -> Result<Vec<Option<ec::Value>>> {
        let entries = self
            .sdos
            .get(&slave)
            .and_then(|info| info.iter().find(|(info, _)| info.idx == idx))
            .map(|(_, entries)| entries)
            .ok_or(Error::IdxNotFound(slave, idx))?;
        let entries: Vec<Option<(usize, ec::DataType)>> = entries
            .iter()
            .map(|e| {
                e.as_ref().map(|e| {
                    let len = e.bit_len as usize;
                    let byte_count = if len % 8 == 0 { len / 8 } else { (len / 8) + 1 };
                    (byte_count, e.data_type)
                })
            })
            .collect();
        let max_byte_count: usize = entries
            .iter()
            .filter_map(Option::as_ref)
            .map(|(cnt, _)| *cnt)
            .max()
            .unwrap_or(0);
        let buff_size = max_byte_count * entries.len();
        let mut target = vec![0; buff_size];
        let raw = self.read_sdo(
            slave,
            ec::SdoIdx {
                idx,
                sub_idx: ec::SubIdx::from(0),
            },
            true,
            &mut target,
            timeout,
        )?;
        let mut byte_pos = 0;
        let mut values = vec![];
        for e in entries {
            let res = match e {
                Some((cnt, data_type)) => {
                    let raw_value = &raw[byte_pos..byte_pos + cnt];
                    let val = util::value_from_slice(data_type, raw_value)?;
                    byte_pos += cnt;
                    Some(val)
                }
                None => None,
            };
            values.push(res);
        }
        Ok(values)
    }

    pub fn write_sdo(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
        access_complete: bool,
        data: &[u8],
        timeout: Duration,
    ) -> Result<()> {
        let index = u16::from(idx.idx);
        let subindex = u8::from(idx.sub_idx);
        let wkc = self.ctx.sdo_write(
            u16::from(slave) + 1,
            index,
            subindex,
            access_complete,
            data,
            timeout,
        );

        if wkc <= 0 {
            let errs = self.ctx_errors();
            log::debug!("Context errors: {:?}", errs);
            return Err(Error::WriteSdo(slave, idx));
        }
        Ok(())
    }

    pub fn write_sdo_entry(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
        value: ec::Value,
        timeout: Duration,
    ) -> Result<()> {
        let index = u16::from(idx.idx);
        let subindex = u8::from(idx.sub_idx);
        let data = util::value_to_bytes(value)?;
        let wkc = self
            .ctx
            .sdo_write(u16::from(slave) + 1, index, subindex, false, &data, timeout);

        if wkc <= 0 {
            let errs = self.ctx_errors();
            log::debug!("Context errors: {:?}", errs);
            return Err(Error::WriteSdo(slave, idx));
        }
        Ok(())
    }

    fn ctx_errors(&mut self) -> Vec<ctx::Error> {
        let mut errors = vec![];
        while let Some(e) = self.ctx.pop_error() {
            errors.push(e);
        }
        errors
    }
}

fn access_from_u16(x: u16) -> ec::SdoEntryAccess {
    const RD_P: u16 = 0b_0000_0001; // Bit 0
    const RD_S: u16 = 0b_0000_0010; // Bit 1
    const RD_O: u16 = 0b_0000_0100; // Bit 2
    const WR_P: u16 = 0b_0000_1000; // Bit 3
    const WR_S: u16 = 0b_0001_0000; // Bit 4
    const WR_O: u16 = 0b_0010_0000; // Bit 5
                                    // Bit 6 RX PDO map
                                    // Bit 7 TX PDO map

    let p = access(x & RD_P > 0, x & WR_P > 0);
    let s = access(x & RD_S > 0, x & WR_S > 0);
    let o = access(x & RD_O > 0, x & WR_O > 0);

    ec::SdoEntryAccess {
        pre_op: p,
        safe_op: s,
        op: o,
    }
}

fn access(read: bool, write: bool) -> ec::Access {
    match (read, write) {
        (true, false) => ec::Access::ReadOnly,
        (false, true) => ec::Access::WriteOnly,
        (true, true) => ec::Access::ReadWrite,
        _ => ec::Access::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_access_type_from_u8() {
        assert_eq!(
            access_from_u16(0),
            ec::SdoEntryAccess {
                pre_op: ec::Access::Unknown,
                safe_op: ec::Access::Unknown,
                op: ec::Access::Unknown,
            }
        );
        assert_eq!(
            access_from_u16(0b_0000_0001),
            ec::SdoEntryAccess {
                pre_op: ec::Access::ReadOnly,
                safe_op: ec::Access::Unknown,
                op: ec::Access::Unknown,
            }
        );
        assert_eq!(
            access_from_u16(0b_0000_1001),
            ec::SdoEntryAccess {
                pre_op: ec::Access::ReadWrite,
                safe_op: ec::Access::Unknown,
                op: ec::Access::Unknown,
            }
        );
        assert_eq!(
            access_from_u16(0b_0001_1101),
            ec::SdoEntryAccess {
                pre_op: ec::Access::ReadWrite,
                safe_op: ec::Access::WriteOnly,
                op: ec::Access::ReadOnly,
            }
        );
    }
}
