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

pub struct Master {
    ctx: Box<ctx::Ctx>,
    sdos: Vec<ec::ObjectDict>,
    pdo_mapping: ec::PdoMapping,
}

impl Master {
    pub fn try_new<S: Into<String>>(iface: S) -> Result<Self> {
        let mut master = Self {
            ctx: Box::new(ctx::Ctx::default()),
            sdos: vec![],
            pdo_mapping: Default::default(),
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
        Master::from_ptr_with_caches(ctx_ptr, vec![], Default::default())
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr_with_caches(
        ctx_ptr: *mut ctx::Ctx,
        sdos: Vec<ec::ObjectDict>,
        pdo_mapping: ec::PdoMapping,
    ) -> Self {
        Master {
            ctx: Box::from_raw(ctx_ptr),
            sdos,
            pdo_mapping,
        }
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn sdo_info_cache(&self) -> &[ec::ObjectDict] {
        &self.sdos
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn pdo_info_cache(&self) -> &ec::PdoMapping {
        &self.pdo_mapping
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
        self.sdos = self.scan_slave_objects()?;
        self.pdo_mapping = self.coe_pdo_info()?;
        Ok(())
    }

    fn scan_slave_objects(&mut self) -> Result<Vec<ec::ObjectDict>> {
        log::debug!("Fetch SDO info");
        (0..self.ctx.slave_count())
            .into_iter()
            .map(|i| ec::SlavePos::from(i as u16))
            .map(|pos| self.read_od_list(pos))
            .collect()
    }

    fn coe_pdo_info(&mut self) -> Result<ec::PdoMapping> {
        log::debug!("Fetch PDO mapping according to CoE");
        let mut mappings = vec![];

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

            let sm_types = self.sm_types(slave_pos, sm_cnt)?;

            for (sm, sm_type) in &sm_types {
                log::debug!(
                    "SM {} has type {} (= {:?})",
                    u8::from(*sm),
                    u8::from(*sm_type),
                    sm_type
                );
            }

            let mut pdo_mapping = vec![];

            for t in &[ec::SmType::Outputs, ec::SmType::Inputs] {
                for (sm, sm_type) in sm_types.iter().filter(|(_, sm_type)| sm_type == t) {
                    log::debug!("Check PDO assignment for {:?}", sm);
                    let pdo_assign = self.si_pdo_assign(slave_pos, *sm, *sm_type)?;
                    log::debug!(
                        "Slave {}: {:?} (type: {:?}): read the assigned PDOs",
                        slave,
                        sm,
                        sm_type
                    );
                    pdo_mapping.push(pdo_assign);
                }
            }
            mappings.push(pdo_mapping);
        }
        Ok(ec::PdoMapping(mappings))
    }

    fn sm_types(&self, slave: ec::SlavePos, sm_cnt: u8) -> Result<Vec<(ec::SmIdx, ec::SmType)>> {
        let mut sm_types = vec![];
        for sm in 2..=sm_cnt {
            let sm_type_id = self
                .slaves()
                .get(usize::from(slave))
                .and_then(|s: &ctx::Slave| s.sm_type().get(sm as usize).cloned())
                .ok_or(Error::InvalidSmType)?;

            let sm_type = ec::SmType::try_from(sm_type_id)?;
            if sm == 2 && sm_type == ec::SmType::MbxRd {
                log::warn!(
                    "SM2 has type 2 == mailbox out, this is a bug in {:?}!",
                    slave
                );
                continue;
            }
            sm_types.push((ec::SmIdx::from(sm), sm_type));
        }
        Ok(sm_types)
    }

    /// Read PDO assign structure
    fn si_pdo_assign(
        &mut self,
        slave: ec::SlavePos,
        sm: ec::SmIdx,
        sm_type: ec::SmType,
    ) -> Result<ec::PdoAssignment> {
        let idx = SDO_IDX_PDO_ASSIGN + u8::from(sm) as u16;

        let mut val = [0];
        self.read_sdo(
            slave,
            ec::EntryIdx::new(idx, 0),
            false,
            &mut val,
            DEFAULT_SDO_TIMEOUT,
        )?;
        let pdo_cnt = val[0];
        log::debug!("Available PDO entries in {:#X}: {}", idx, pdo_cnt);

        let mut pdos = vec![];

        // read all PDO's
        for pdo_sub in 1..=pdo_cnt {
            log::debug!("{:?}: read PDO IDX from {:#X}.{:#X}", slave, idx, pdo_sub);

            let mut val = [0; 2];
            self.read_sdo(
                slave,
                ec::EntryIdx::new(idx, pdo_sub),
                false,
                &mut val,
                DEFAULT_SDO_TIMEOUT,
            )?;
            let pdo_idx = u16::from_ne_bytes(val);
            log::debug!("PDO IDX is {:#X}", pdo_idx);

            log::debug!("{:?}: read PDO count from 0x{:X}.0x{:X}", slave, pdo_idx, 0);
            let mut val = [0];
            self.read_sdo(
                slave,
                ec::EntryIdx::new(pdo_idx, 0),
                false,
                &mut val,
                DEFAULT_SDO_TIMEOUT,
            )?;
            let pdo_entry_cnt = val[0];
            log::debug!("... PDO count is {}", pdo_entry_cnt);

            let mut pdo_entries = vec![];

            for entry_sub in 1..=pdo_entry_cnt {
                let pdo_info = self.pdo_entry_info(slave, ec::EntryIdx::new(pdo_idx, entry_sub))?;
                pdo_entries.push(pdo_info);
            }
            let pdo_info = ec::PdoInfo {
                idx: pdo_idx.into(),
                entries: pdo_entries,
            };
            pdos.push(pdo_info);
        }
        Ok(ec::PdoAssignment { sm, sm_type, pdos })
    }

    fn pdo_entry_info(
        &mut self,
        slave: ec::SlavePos,
        pdo_data_sdo_idx: ec::EntryIdx,
    ) -> Result<ec::PdoEntryInfo> {
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
        let sdo = ec::EntryIdx::new(obj_idx, obj_subidx);
        Ok(ec::PdoEntryInfo { sdo, bit_len })
    }

    fn sm_comm_type_sdo(&mut self, slave: ec::SlavePos, sub_idx: u8) -> Result<u8> {
        let mut val = [0];
        self.read_sdo(
            slave,
            ec::EntryIdx {
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

    fn read_od_desc(&mut self, item: u16, od_list: &mut ctx::OdList) -> Result<ec::ObjectInfo> {
        let res = self.ctx.read_od_description(item, od_list);
        let pos = ec::ObjectPos::from(item);
        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::ReadOdDesc(pos));
        }
        let i = item as usize;
        let idx = ec::Idx::from(od_list.indexes()[i]);
        let object_code = Some(od_list.object_codes()[i]);
        let name = od_list.names()[i].clone();
        let max_sub_idx = ec::SubIdx::from(od_list.max_subs()[i]);
        let info = ec::ObjectInfo {
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
        let pos = ec::ObjectPos::from(item);
        if res <= 0 {
            log::debug!("Context errors: {:?}", self.ctx_errors());
            return Err(Error::ReadOeList(pos));
        }
        Ok(oe_list)
    }

    pub fn read_od_list(&mut self, slave: ec::SlavePos) -> Result<ec::ObjectDict> {
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
        let mut dict = ec::ObjectDict::default();
        for i in 0..od_list.entries() {
            let sdo_info = self.read_od_desc(i as u16, &mut od_list)?;
            let oe_list = self.read_oe_list(i as u16, &mut od_list)?;
            for j in 0..=u8::from(sdo_info.max_sub_idx) as usize {
                let dt = oe_list.data_types()[j];
                let bit_len = oe_list.bit_lengths()[j];
                let data_type = ec::DataType::from_u16(dt);
                let entry_idx = ec::EntryIdx {
                    idx: sdo_info.idx,
                    sub_idx: ec::SubIdx::from(j as u8),
                };
                let pos = None; // TODO
                let access = oe_list.object_access().get(j).cloned().map(access_from_u16);
                let name = oe_list.names()[j].clone();
                let info = ec::EntryInfo {
                    data_type,
                    bit_len,
                    access,
                    name,
                    entry_idx,
                    pos,
                };
                dict.add_entry(info);
            }
            dict.add_obj(sdo_info);
        }
        Ok(dict)
    }

    pub fn read_sdo<'t>(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::EntryIdx,
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
        idx: ec::EntryIdx,
        timeout: Duration,
    ) -> Result<ec::Value> {
        let (dt, len) = self
            .sdos
            .get(usize::from(slave))
            .and_then(|dict| dict.entries.get(&idx))
            .and_then(|info| info.data_type.map(|dt| (dt, info.bit_len as usize)))
            .ok_or(Error::SubIdxNotFound(slave, idx))?;
        let byte_count = byte_cnt(len);
        let mut target = vec![0; byte_count];
        let raw_value = self.read_sdo(slave, idx, false, &mut target, timeout)?;
        debug_assert!(!raw_value.is_empty());
        log::debug!(
            "SDO raw data at {:?} {:#X}.{:#X} is {:?}",
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
    ) -> Result<Vec<ec::Value>> {
        let entries = self
            .sdos
            .get(usize::from(slave))
            .map(|dict| dict.entries.iter().filter(|(e, _)| e.idx == idx))
            .ok_or(Error::IdxNotFound(slave, idx))?;
        let entries: Vec<(usize, ec::DataType)> = entries
            .map(|(_, e)| {
                let len = e.bit_len as usize;
                let byte_count = if len % 8 == 0 { len / 8 } else { (len / 8) + 1 };
                (
                    byte_count,
                    e.data_type.unwrap_or_else(|| {
                        let fallback = ec::DataType::Raw;
                        log::debug!(
                            "Unknown DataType for {:?}: use {:?} as fallback",
                            e.entry_idx,
                            fallback
                        );
                        ec::DataType::Raw
                    }),
                )
            })
            .collect();
        let max_byte_count: usize = entries.iter().map(|(cnt, _)| *cnt).max().unwrap_or(0);
        let buff_size = max_byte_count * entries.len();
        let mut target = vec![0; buff_size];
        let raw = self.read_sdo(
            slave,
            ec::EntryIdx {
                idx,
                sub_idx: ec::SubIdx::from(0),
            },
            true,
            &mut target,
            timeout,
        )?;
        let mut byte_pos = 0;
        let mut values = vec![];
        for (cnt, data_type) in entries {
            let raw_value = &raw[byte_pos..byte_pos + cnt];
            let val = util::value_from_slice(data_type, raw_value, 0)?;
            byte_pos += cnt;
            values.push(val);
        }
        Ok(values)
    }

    pub fn write_sdo(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::EntryIdx,
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
        idx: ec::EntryIdx,
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

    pub fn pdo_values(&self) -> impl Iterator<Item = Vec<(ec::EntryIdx, ec::Value)>> + '_ {
        self.slaves()
            .iter()
            .enumerate()
            .filter_map(move |(i, slave)| {
                let slave_pos = ec::SlavePos::new(i as u16);
                if let Some(assignments) = self.pdo_mapping.get(slave_pos) {
                    let mut values = vec![];
                    let dict = self.sdos.get(i);
                    for a in assignments {
                        match a.sm_type {
                            ec::SmType::Outputs => {
                                let outputs: &[u8] = slave.outputs();
                                values.extend_from_slice(&process_data(outputs, &a.pdos, dict));
                            }
                            ec::SmType::Inputs => {
                                let inputs: &[u8] = slave.inputs();
                                values.extend_from_slice(&process_data(inputs, &a.pdos, dict));
                            }
                            _ => {}
                        }
                    }
                    Some(values)
                } else {
                    log::warn!("Could not find PDO meta data for Slave {}", i);
                    None
                }
            })
    }

    pub fn set_pdo_value(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::EntryIdx,
        v: ec::Value,
    ) -> Result<()> {
        let pdos = &self
            .pdo_mapping
            .outputs(slave)
            .ok_or(Error::NoOutputPdos(slave))?
            .pdos;

        let offsets = pdo_offsets(pdos);

        let (_, offset) = offsets
            .iter()
            .find(|(x, _)| *x == idx)
            .ok_or(Error::PdoEntryNotFound(idx))?;

        let e = pdos
            .iter()
            .find(|info| info.idx == idx.idx)
            .map(|info| &info.entries)
            .and_then(|entries| entries.iter().find(|e| e.sdo == idx))
            .ok_or(Error::PdoEntryNotFound(idx))?;

        let data_type = self
            .sdos
            .get(usize::from(slave))
            .and_then(|dict| dict.entries.iter().find(|(idx, _)| *idx == &e.sdo))
            .and_then(|(_, e)| e.data_type)
            .ok_or(Error::UnexpectedDataType)?;

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

const fn byte_cnt(bits: usize) -> usize {
    if bits % 8 == 0 {
        bits / 8
    } else {
        (bits / 8) + 1
    }
}

const fn access_from_u16(x: u16) -> ec::EntryAccess {
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

    ec::EntryAccess {
        pre_op: p,
        safe_op: s,
        op: o,
    }
}

const fn access(read: bool, write: bool) -> ec::Access {
    match (read, write) {
        (true, false) => ec::Access::ReadOnly,
        (false, true) => ec::Access::WriteOnly,
        (true, true) => ec::Access::ReadWrite,
        _ => ec::Access::Unknown,
    }
}

fn process_data(
    data: &[u8],
    pdos: &[ec::PdoInfo],
    dict: Option<&ec::ObjectDict>,
) -> Vec<(ec::EntryIdx, ec::Value)> {
    let mut values = vec![];
    let mut bit_offset = 0;

    for pdo in pdos {
        for entry in &pdo.entries {
            let len = byte_cnt(entry.bit_len as usize);
            let byte = bit_offset / 8;
            let raw = &data[byte..byte + len];
            let data_type = dict
                .and_then(|dict| dict.entries.get(&entry.sdo))
                .and_then(|sdo| sdo.data_type)
                .unwrap_or_else(|| {
                    let fallback = ec::DataType::Raw;
                    log::debug!(
                        "No data type info found for {:?} : use {:?} as fallback",
                        entry.sdo,
                        fallback
                    );
                    fallback
                });
            match util::value_from_slice(data_type, raw, bit_offset % 8) {
                Ok(val) => {
                    values.push((entry.sdo, val));
                }
                Err(err) => {
                    log::warn!("{}", err);
                }
            }
            bit_offset += entry.bit_len;
        }
    }
    values
}

fn pdo_offsets(pdos: &[ec::PdoInfo]) -> Vec<(ec::EntryIdx, ec::Offset)> {
    let mut bit_offset = 0;
    pdos.iter()
        .flat_map(|pdo| {
            pdo.entries
                .iter()
                .map(|entry| {
                    let byte = (bit_offset / 8) as usize;
                    let bit = (bit_offset % 8) as u32;
                    let offset = ec::Offset { bit, byte };
                    bit_offset += entry.bit_len;
                    (entry.sdo, offset)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_pdo_data() {
        let v = (7.7_f32).to_ne_bytes();
        let raw_data = &[
            v[0],
            v[1],
            v[2],
            v[3],         // 0x7001, SubIdx: 0x03
            0b_0000_0010, // 0x8010, SubIdx: 0x02,0x06
        ];

        let float_03 = ec::EntryInfo {
            entry_idx: ec::EntryIdx::new(0x7001, 0x03),
            access: None,
            bit_len: 32,
            data_type: Some(ec::DataType::F32),
            name: "a float value".into(),
            pos: None,
        };

        let bit_02 = ec::EntryInfo {
            entry_idx: ec::EntryIdx::new(0x8010, 0x02),
            access: None,
            bit_len: 1,
            data_type: Some(ec::DataType::Bool),
            name: "a bit value".into(),
            pos: None,
        };

        let bit_06 = ec::EntryInfo {
            entry_idx: ec::EntryIdx::new(0x8010, 0x06),
            access: None,
            bit_len: 1,
            data_type: Some(ec::DataType::Bool),
            name: "a bit value".into(),
            pos: None,
        };

        let float_03_entry = ec::PdoEntryInfo {
            bit_len: float_03.bit_len as usize,
            sdo: float_03.entry_idx,
        };
        let float_pdo = ec::PdoInfo {
            idx: ec::Idx::new(0x1600),
            entries: vec![float_03_entry],
        };
        let bit_02_entry = ec::PdoEntryInfo {
            bit_len: bit_02.bit_len as usize,
            sdo: bit_02.entry_idx,
        };
        let bit_06_entry = ec::PdoEntryInfo {
            bit_len: bit_06.bit_len as usize,
            sdo: bit_06.entry_idx,
        };
        let bit_pdo = ec::PdoInfo {
            idx: ec::Idx::new(0x1601),
            entries: vec![bit_02_entry, bit_06_entry],
        };

        let pdo_mapping = ec::PdoMapping(vec![vec![ec::PdoAssignment {
            sm: ec::SmIdx::new(2),
            sm_type: ec::SmType::Outputs,
            pdos: vec![float_pdo, bit_pdo],
        }]]);

        let mut dict = ec::ObjectDict::default();

        dict.add_entry(float_03);
        dict.add_entry(bit_02);
        dict.add_entry(bit_06);

        let values = super::process_data(
            raw_data,
            &pdo_mapping.outputs(0.into()).unwrap().pdos,
            Some(&dict),
        );

        assert_eq!(values.len(), 3);

        assert_eq!(values[0].1, ec::Value::F32(7.7));
        assert_eq!(values[1].1, ec::Value::Bool(false));
        assert_eq!(values[2].1, ec::Value::Bool(true));

        assert_eq!(values[0].0, ec::EntryIdx::new(0x7001, 0x03));
        assert_eq!(values[1].0, ec::EntryIdx::new(0x8010, 0x02));
        assert_eq!(values[2].0, ec::EntryIdx::new(0x8010, 0x06));
    }

    #[test]
    fn pdo_offsets() {
        let float_03_entry = ec::PdoEntryInfo {
            bit_len: 32,
            sdo: ec::EntryIdx::new(0x7001, 0x03),
        };
        let float_pdo = ec::PdoInfo {
            idx: ec::Idx::new(0x1600),
            entries: vec![float_03_entry],
        };
        let bit_02_entry = ec::PdoEntryInfo {
            bit_len: 1,
            sdo: ec::EntryIdx::new(0x8010, 0x2),
        };
        let bit_06_entry = ec::PdoEntryInfo {
            bit_len: 1,
            sdo: ec::EntryIdx::new(0x8010, 0x6),
        };
        let bit_pdo = ec::PdoInfo {
            idx: ec::Idx::new(0x1601),
            entries: vec![bit_02_entry, bit_06_entry],
        };

        let pdo_mapping = ec::PdoMapping(vec![vec![ec::PdoAssignment {
            sm: ec::SmIdx::new(2),
            sm_type: ec::SmType::Outputs,
            pdos: vec![float_pdo, bit_pdo],
        }]]);
        let offsets = super::pdo_offsets(&pdo_mapping.outputs(0_u16.into()).unwrap().pdos);

        assert_eq!(offsets.len(), 3);
        assert_eq!(offsets[0].1.bit, 0);
        assert_eq!(offsets[0].1.byte, 0);

        assert_eq!(offsets[1].1.bit, 0);
        assert_eq!(offsets[1].1.byte, 4);

        assert_eq!(offsets[2].1.bit, 1);
        assert_eq!(offsets[2].1.byte, 4);
    }

    #[test]
    fn get_access_type_from_u8() {
        assert_eq!(
            access_from_u16(0),
            ec::EntryAccess {
                pre_op: ec::Access::Unknown,
                safe_op: ec::Access::Unknown,
                op: ec::Access::Unknown,
            }
        );
        assert_eq!(
            access_from_u16(0b_0000_0001),
            ec::EntryAccess {
                pre_op: ec::Access::ReadOnly,
                safe_op: ec::Access::Unknown,
                op: ec::Access::Unknown,
            }
        );
        assert_eq!(
            access_from_u16(0b_0000_1001),
            ec::EntryAccess {
                pre_op: ec::Access::ReadWrite,
                safe_op: ec::Access::Unknown,
                op: ec::Access::Unknown,
            }
        );
        assert_eq!(
            access_from_u16(0b_0001_1101),
            ec::EntryAccess {
                pre_op: ec::Access::ReadWrite,
                safe_op: ec::Access::WriteOnly,
                op: ec::Access::ReadOnly,
            }
        );
    }
}
