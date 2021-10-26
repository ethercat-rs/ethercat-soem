#[macro_use]
extern crate num_derive;

use ethercat_soem_ctx as ctx;
use ethercat_types as ec;
use num_traits::cast::FromPrimitive;
use std::{convert::TryFrom, ffi::CString, time::Duration};

mod al_status;
mod error;
mod util;

pub use self::{al_status::*, error::Error};

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_micros(2_000);
const DEFAULT_SDO_TIMEOUT: Duration = Duration::from_millis(3_000);

const MAX_SM_CNT: u8 = 8;

const SDO_IDX_PDO_ASSIGN: u16 = 0x1C10;
const SDO_IDX_SM_COMM_TYPE: ec::Idx = ec::Idx::new(0x1C00);

const EC_NOFRAME: i32 = -1;

type Result<T> = std::result::Result<T, Error>;

type SdoInfo = Vec<(ec::SdoInfo, Vec<Option<ec::SdoEntryInfo>>)>;
type PdoInfo = Vec<(ec::PdoInfo, Vec<PdoEntryInfo>)>;

// TODO: merge into ec::PdoEntryInfo
#[derive(Debug, Clone, PartialEq)]
pub struct PdoEntryInfo {
    pub idx: ec::PdoEntryIdx,
    pub pos: ec::PdoEntryPos,
    pub data_type: ec::DataType,
    pub offset: ec::Offset,
    pub bit_len: usize,
    pub name: String,
    pub sm: ec::SmType,
    pub sdo: ec::SdoIdx,
}

pub struct Master {
    ctx: Box<ctx::Ctx>,
    sdos: Vec<SdoInfo>,
    pdos: Vec<PdoInfo>,
}

impl Master {
    pub fn try_new<S: Into<String>>(iface: S) -> Result<Self> {
        let mut master = Self {
            ctx: Box::new(ctx::Ctx::default()),
            sdos: vec![],
            pdos: vec![],
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
    pub fn io_map(&mut self) -> &mut [u8] {
        &mut self.ctx.io_map
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr(ctx_ptr: *mut ctx::Ctx) -> Self {
        Master::from_ptr_with_caches(ctx_ptr, vec![], vec![])
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr_with_caches(
        ctx_ptr: *mut ctx::Ctx,
        sdos: Vec<SdoInfo>,
        pdos: Vec<PdoInfo>,
    ) -> Self {
        Master {
            ctx: Box::from_raw(ctx_ptr),
            sdos,
            pdos,
        }
    }

    #[doc(hidden)]
    /// Drop a secondary instance safely
    ///
    /// Drop an instance that has been created with `from_ptr` or `from_ptr_with_caches`
    /// from a shared context pointer that is dropped later. Otherwise dropping this
    /// instance would result in a double-free.
    ///
    /// TODO: Avoid all these ugly hacks!
    pub fn leak_ptr(self) {
        let Self {
            ctx,
            ..
        } = self;
        Box::leak(ctx);
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn sdo_info_cache(&self) -> &[SdoInfo] {
        &self.sdos
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn pdo_info_cache(&self) -> &[PdoInfo] {
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
        let res = self.ctx.config_dc();
        if res == 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::CfgDc);
        }
        let group = 0;
        let io_map_size = self.ctx.config_map_group(group);
        if io_map_size <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::CfgMapGroup);
        }
        let expected_wkc = self.group_outputs_wkc(0)? * 2 + self.group_inputs_wkc(0)?;
        log::debug!("Expected working counter = {}", expected_wkc);
        self.scan_slave_objects()?;
        self.pdos = self.coe_pdo_info()?;
        Ok(())
    }

    fn scan_slave_objects(&mut self) -> Result<()> {
        log::debug!("Fetch SDO info");
        for i in 0..self.ctx.slave_count() {
            let pos = ec::SlavePos::from(i as u16);
            let sdo_info = self.read_od_list(pos)?;
            self.sdos.push(sdo_info);
        }
        Ok(())
    }

    fn coe_pdo_info(&mut self) -> Result<Vec<PdoInfo>> {
        log::debug!("Fetch PDO mapping according to CoE");
        let mut res = vec![];

        for slave in 0..self.ctx.slave_count() as u16 {
            let slave_pos = ec::SlavePos::new(slave);

            let obj_cnt = self.sm_comm_type_sdo(slave_pos, 0)?;
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
                let sm_type_id = self
                    .slaves()
                    .get(slave as usize)
                    .and_then(|s: &ctx::Slave| s.sm_type().get(sm as usize).cloned())
                    .ok_or(Error::InvalidSmType)?;

                let sm_type = ec::SmType::try_from(sm_type_id)?;
                if sm == 2 && sm_type == ec::SmType::MbxRd {
                    log::warn!(
                        "SM2 has type 2 == mailbox out, this is a bug in {:?}!",
                        slave_pos
                    );
                    continue;
                }
                sm_types.push((sm, sm_type));
            }
            for (sm, sm_type) in &sm_types {
                log::debug!(
                    "SM {} has type {} (= {:?})",
                    sm,
                    u8::from(*sm_type),
                    sm_type
                );
            }
            let mut pdo_cnt_offset = 0;
            let mut pdo_entry_cnt_offset = 0;

            for t in &[ec::SmType::Outputs, ec::SmType::Inputs] {
                for (sm, sm_type) in sm_types.iter().filter(|(_, sm_type)| sm_type == t) {
                    log::debug!("Check PDO assignment for SM {}", sm);
                    let pdo_assign = self.si_pdo_assign(
                        slave_pos,
                        *sm,
                        *sm_type,
                        pdo_cnt_offset,
                        pdo_entry_cnt_offset,
                    )?;
                    log::debug!(
                        "Slave {}: SM {} (type: {:?}): read the assigned PDOs",
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
            res.push(pdo_info);
        }
        Ok(res)
    }

    /// Read PDO assign structure
    fn si_pdo_assign(
        &mut self,
        slave: ec::SlavePos,
        sm: u8,
        sm_type: ec::SmType,
        pdo_pos_offset: u8,
        pdo_entry_pos_offset: u8,
    ) -> Result<PdoInfo> {
        let idx = SDO_IDX_PDO_ASSIGN + sm as u16;

        let mut val = [0];
        self.read_sdo(
            slave,
            ec::SdoIdx::new(idx, 0),
            false,
            &mut val,
            DEFAULT_SDO_TIMEOUT,
        )?;
        let pdo_cnt = val[0];
        log::debug!("Available PDO entries in 0x{:X}: {}", idx, pdo_cnt);

        let mut pdo_entry_pos = pdo_entry_pos_offset;
        let mut bit_offset = 0_usize;

        let mut pdos = vec![];

        // read all PDO's
        for pdo_sub in 1..=pdo_cnt {
            let pdo_pos = pdo_pos_offset + pdo_sub - 1;
            log::debug!("{:?}: read PDO IDX from 0x{:X}.0x{:X}", slave, idx, pdo_sub);

            let mut val = [0; 2];
            self.read_sdo(
                slave,
                ec::SdoIdx::new(idx, pdo_sub),
                false,
                &mut val,
                DEFAULT_SDO_TIMEOUT,
            )?;
            let pdo_idx = u16::from_ne_bytes(val);
            log::debug!("PDO IDX is 0x{:X}", pdo_idx);

            log::debug!("{:?}: read PDO count from 0x{:X}.0x{:X}", slave, pdo_idx, 0);
            let mut val = [0];
            self.read_sdo(
                slave,
                ec::SdoIdx::new(pdo_idx, 0),
                false,
                &mut val,
                DEFAULT_SDO_TIMEOUT,
            )?;
            let pdo_entry_cnt = val[0];
            log::debug!("... PDO count is {}", pdo_entry_cnt);

            let mut pdo_entries = vec![];

            for entry_sub in 1..=pdo_entry_cnt {
                let pdo_data_sdo_idx = ec::SdoIdx::new(pdo_idx, entry_sub);
                let mut val = [0; 4];
                self.read_sdo(
                    slave,
                    pdo_data_sdo_idx,
                    false,
                    &mut val,
                    DEFAULT_SDO_TIMEOUT,
                )?;

                let data = u32::from_ne_bytes(val);
                let bit_len = (data & 0x_00FF) as usize;
                let obj_idx = (data >> 16) as u16;
                let obj_subidx = ((data >> 8) & 0x_0000_00FF) as u8;

                let sdo = ec::SdoIdx::new(obj_idx, obj_subidx);
                let sdo_entry = self.cached_sdo_entry(slave, sdo);

                let (name, data_type) = match sdo_entry {
                    Some(e) => {
                        debug_assert_eq!(bit_len as u16, e.bit_len);
                        (e.description.clone(), e.data_type)
                    }
                    None => {
                        log::warn!("Could not find SDO ({:?}) entry description", sdo);
                        (String::new(), ec::DataType::Raw)
                    }
                };
                let idx = ec::PdoEntryIdx::new(pdo_idx, entry_sub);
                let pos = ec::PdoEntryPos::new(pdo_entry_pos);

                let offset = ec::Offset {
                    byte: bit_offset / 8,
                    bit: (bit_offset % 8) as u32,
                };
                bit_offset += bit_len;

                let pdo_entry_info = PdoEntryInfo {
                    bit_len,
                    data_type,
                    name,
                    sdo,
                    idx,
                    sm: sm_type,
                    pos,
                    offset,
                };

                pdo_entries.push(pdo_entry_info);
                pdo_entry_pos += 1;
            }
            if let Some(e) = pdo_entries.get(0) {
                let sdo_info = self.cached_sdo_info(slave, e.sdo.idx);

                let name = match sdo_info {
                    Some(info) => info.name.clone(),
                    None => {
                        log::warn!("Could not find SDO ({:?}) name", e.sdo.idx);
                        String::new()
                    }
                };

                let pdo_info = ec::PdoInfo {
                    sm: ec::SmIdx::new(sm),
                    pos: ec::PdoPos::new(pdo_pos),
                    idx: pdo_idx.into(),
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
            .get(usize::from(slave))
            .and_then(|sdos| sdos.iter().find(|(info, _entries)| info.idx == idx.idx))
            .map(|(_, x)| x)
            .and_then(|entries| entries.get(u8::from(idx.sub_idx) as usize))
            .and_then(Option::as_ref)
    }

    fn cached_sdo_info(&mut self, slave: ec::SlavePos, idx: ec::Idx) -> Option<&ec::SdoInfo> {
        self.sdos
            .get(usize::from(slave))
            .and_then(|sdos| sdos.iter().find(|(info, _entries)| info.idx == idx))
            .map(|(x, _)| x)
    }

    fn sm_comm_type_sdo(&mut self, slave: ec::SlavePos, sub_idx: u8) -> Result<u8> {
        let mut val = [0];
        self.read_sdo(
            slave,
            ec::SdoIdx {
                idx: SDO_IDX_SM_COMM_TYPE,
                sub_idx: sub_idx.into(),
            },
            false,
            &mut val,
            DEFAULT_SDO_TIMEOUT,
        )?;
        Ok(val[0])
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

    pub fn check_states(&mut self, state: ec::AlState, timeout: Duration) -> Result<ec::AlState> {
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
            log::debug!(
                "Current state {:?} does not match expected state {:?}",
                found_state,
                state
            );
        }
        Ok(found_state)
    }

    pub fn slaves(&self) -> &[ctx::Slave] {
        let cnt = self.ctx.slave_count();
        &self.ctx.slaves()[1..=cnt]
    }

    pub fn slaves_mut(&mut self) -> &mut [ctx::Slave] {
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
            .get(usize::from(slave))
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
        log::debug!(
            "SDO raw data at {:?} 0x{:X}.{:X} is {:?}",
            slave,
            u16::from(idx.idx),
            u8::from(idx.sub_idx),
            raw_value
        );
        Ok(util::value_from_slice(dt, raw_value, 0)?)
    }

    pub fn read_sdo_complete(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::Idx,
        timeout: Duration,
    ) -> Result<Vec<Option<ec::Value>>> {
        let entries = self
            .sdos
            .get(usize::from(slave))
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
                    let val = util::value_from_slice(data_type, raw_value, 0)?;
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
        log::debug!(
            "Write SDO raw data {:?} to {:?} 0x{:X}.{:X}",
            data,
            slave,
            u16::from(idx.idx),
            u8::from(idx.sub_idx),
        );
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

    pub fn pdo_values(&self) -> Vec<Vec<(ec::Idx, Vec<ec::Value>)>> {
        let mut all_pdos = vec![];
        for (i, slave) in self.slaves().iter().enumerate() {
            let mut slave_pdos = vec![];
            if let Some(pdo_meta_data) = self.pdos.get(i) {
                let inputs: &[u8] = slave.inputs();
                let outputs: &[u8] = slave.outputs();

                for (pdo_info, pdo_entries) in pdo_meta_data {
                    let mut pdos = vec![];
                    for PdoEntryInfo {
                        bit_len,
                        offset,
                        data_type,
                        sm,
                        ..
                    } in pdo_entries
                    {
                        let ec::Offset { byte, bit } = *offset;
                        let slice = match sm {
                            ec::SmType::Inputs => Some(inputs),
                            ec::SmType::Outputs => Some(outputs),
                            _ => None,
                        };
                        match slice {
                            Some(d) => {
                                let len = byte_cnt(*bit_len as usize);
                                let raw = &d[byte..byte + len];
                                match util::value_from_slice(*data_type, raw, bit as usize) {
                                    Ok(val) => {
                                        pdos.push(val);
                                    }
                                    Err(err) => {
                                        log::warn!("{}", err);
                                    }
                                }
                            }
                            None => {
                                log::warn!("Unexpected SM type: {:?}", sm);
                            }
                        }
                    }
                    slave_pdos.push((pdo_info.idx, pdos));
                }
            } else {
                log::warn!("Could not find PDO meta data for Slave {}", i);
            }
            all_pdos.push(slave_pdos);
        }

        all_pdos
    }

    pub fn set_pdo_value(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::PdoEntryIdx,
        v: ec::Value,
    ) -> Result<()> {
        let (data_type, offset) = {
            let e = self
                .pdos
                .get(usize::from(slave))
                .ok_or(Error::SlaveNotFound(slave))?
                .iter()
                .find(|(info, _)| info.idx == idx.idx)
                .and_then(|(_, entries)| entries.iter().find(|e| e.idx == idx))
                .ok_or(Error::PdoEntryNotFound(idx))?;
            if e.sm != ec::SmType::Outputs {
                return Err(Error::InvalidSmType);
            }
            (e.data_type, e.offset)
        };
        let s: &mut ctx::Slave = self
            .slaves_mut()
            .get_mut(usize::from(slave))
            .ok_or(Error::SlaveNotFound(slave))?;
        let bytes = util::value_to_bytes(v)?;

        if data_type == ec::DataType::Bool {
            debug_assert_eq!(bytes.len(), 1);
            let mask = 1 << offset.bit;
            if bytes[0] == 1 {
                s.outputs_mut()[offset.byte] |= mask; // Set Bit
            } else {
                s.outputs_mut()[offset.byte] &= !mask; // Clear Bit
            }
        } else {
            for (i, b) in bytes.into_iter().enumerate() {
                s.outputs_mut()[offset.byte + i] = b;
            }
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

fn byte_cnt(bits: usize) -> usize {
    if bits % 8 == 0 {
        bits / 8
    } else {
        (bits / 8) + 1
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
