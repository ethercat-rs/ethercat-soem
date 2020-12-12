use ethercat_soem_ctx as ctx;
use ethercat_types as ec;
use num_traits::cast::FromPrimitive;
use std::{convert::TryFrom, ffi::CString, time::Duration};

mod error;

pub use error::Error;

const EC_TIMEOUTRET: u64 = 2_000;

type Result<T> = std::result::Result<T, Error>;

pub struct Master(Box<ctx::Ctx>);

impl Master {
    pub fn try_new<S: Into<String>>(iface: S) -> Result<Self> {
        let mut master = Self(Box::new(ctx::Ctx::default()));
        master.init(iface.into())?;
        Ok(master)
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn ptr(&mut self) -> *mut ctx::Ctx {
        let reference: &mut ctx::Ctx = &mut *self.0;
        reference as *mut ctx::Ctx
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr(ctx_ptr: *mut ctx::Ctx) -> Self {
        Master(Box::from_raw(ctx_ptr))
    }

    fn init(&mut self, iface: String) -> Result<()> {
        log::debug!("Initialise SOEM stack: bind socket to {}", iface);
        let iface = CString::new(iface).map_err(|_| Error::Iface)?;
        let res = self.0.init(iface);
        if res <= 0 {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::Init);
        }
        Ok(())
    }

    pub fn auto_config(&mut self) -> Result<()> {
        log::debug!("Find and auto-config slaves");
        let usetable = false;
        let res = self.0.config_init(usetable);
        if res <= 0 {
            log::debug!("{:?}", self.ctx_errors());
            return Err(match res {
                -1 => Error::NoFrame,
                -2 => Error::OtherFrame,
                _ => Error::NoSlaves,
            });
        }
        log::debug!("{} slaves found and configured", self.0.slave_count());
        let group = 0;
        let io_map_size = self.0.config_map_group(group);
        if io_map_size <= 0 {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::CfgMapGroup);
        }
        let res = self.0.config_dc();
        if res == 0 {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::CfgDc);
        }
        Ok(())
    }

    pub fn request_states(&mut self, state: ec::AlState) -> Result<()> {
        log::debug!("wait for all slaves to reach {:?} state", state);
        let s = u8::from(state) as u16;
        for i in 0..=self.0.slave_count() {
            self.0.slaves_mut()[i].set_state(s);
        }
        let wkc = self.0.write_state(0);
        if wkc <= 0 {
            log::debug!("{:?}", self.ctx_errors());
            log::warn!("Could not set state {:?} for slaves", state);
            return Err(Error::SetState);
        }
        Ok(())
    }

    pub fn check_states(&mut self, state: ec::AlState) -> Result<()> {
        let res = self
            .0
            .state_check(0, u8::from(state) as u16, Duration::from_micros(50_000));
        if res == 0 {
            log::debug!("{:?}", self.ctx_errors());
            log::warn!("Could not check state {:?} for slaves", state);
            return Err(Error::CheckState);
        }
        Ok(())
    }

    pub fn slaves(&mut self) -> &mut [ctx::Slave] {
        let cnt = self.0.slave_count();
        &mut self.0.slaves_mut()[1..=cnt]
    }

    pub fn states(&mut self) -> Result<Vec<ec::AlState>> {
        let lowest_state = self.0.read_state();
        if lowest_state <= 0 {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::ReadStates);
        }
        // TODO: check 'ALstatuscode'
        let states = (1..=self.0.slave_count())
            .into_iter()
            .map(|i| self.slave_state(i))
            .collect::<Result<_>>()?;
        Ok(states)
    }

    fn slave_state(&self, slave: usize) -> Result<ec::AlState> {
        ec::AlState::try_from(self.0.slaves()[slave].state() as u8).map_err(|_| Error::AlState)
    }

    pub fn send_processdata(&mut self) -> Result<()> {
        self.0.send_processdata();
        if self.0.is_err() {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::SendProcessData);
        }
        Ok(())
    }

    pub fn recv_processdata(&mut self) -> Result<usize> {
        let wkc = self
            .0
            .receive_processdata(Duration::from_micros(EC_TIMEOUTRET));
        if self.0.is_err() {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::RecvProcessData);
        }
        Ok(wkc as usize)
    }

    pub fn group_outputs_wkc(&mut self, i: usize) -> Result<usize> {
        if i >= self.0.groups().len() || i >= self.max_group() {
            if self.0.is_err() {
                log::debug!("{:?}", self.ctx_errors());
            }
            return Err(Error::GroupId);
        }
        Ok(self.0.groups()[i].outputs_wkc() as usize)
    }

    pub fn group_inputs_wkc(&mut self, i: usize) -> Result<usize> {
        if i >= self.0.groups().len() || i >= self.max_group() {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::GroupId);
        }
        Ok(self.0.groups()[i].inputs_wkc() as usize)
    }

    pub fn slave_count(&self) -> usize {
        self.0.slave_count() as usize
    }

    pub fn max_group(&self) -> usize {
        self.0.max_group() as usize
    }

    pub fn dc_time(&self) -> i64 {
        self.0.dc_time()
    }

    fn read_od_desc(&mut self, item: u16, od_list: &mut ctx::OdList) -> Result<ec::SdoInfo> {
        let res = self.0.read_od_description(item, od_list);
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            log::debug!("{:?}", self.ctx_errors());
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
        let res = self.0.read_oe(item, od_list, &mut oe_list);
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            log::debug!("{:?}", self.ctx_errors());
            return Err(Error::ReadOeList(pos));
        }
        Ok(oe_list)
    }

    pub fn read_od_list(
        &mut self,
        slave: ec::SlavePos,
    ) -> Result<Vec<(ec::SdoInfo, Vec<ec::SdoEntryInfo>)>> {
        let mut od_list = ctx::OdList::default();
        let res = self.0.read_od_list(u16::from(slave) + 1, &mut od_list);

        if res <= 0 {
            log::debug!("{:?}", self.ctx_errors());
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
                if oe_list.data_types()[j] > 0 && oe_list.bit_lengths()[j] > 0 {
                    let dt = oe_list.data_types()[j];
                    let data_type = ec::DataType::from_u16(dt).unwrap_or_else(|| {
                        log::warn!("Unknown DataType {}: use RAW as fallback", dt);
                        ec::DataType::Raw
                    });
                    let bit_len = oe_list.bit_lengths()[j];
                    let access = access_from_u16(oe_list.object_access()[j]);
                    let description = oe_list.names()[j].clone();
                    let entry_info = ec::SdoEntryInfo {
                        data_type,
                        bit_len,
                        access,
                        description,
                    };
                    entries.push(entry_info);
                } else {
                    log::debug!("Invalid SDO entry: {:?} item {}", sdo_info.pos, j);
                }
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

        let (wkc, slice) = self.0.sdo_read(
            u16::from(slave) + 1,
            index,
            subindex,
            access_complete,
            target,
            timeout,
        );
        if wkc <= 0 {
            let errs = self.ctx_errors();
            log::debug!("{:?}", errs);
            for e in errs {
                if e.err_type == ctx::ErrType::Packet && e.abort_code == 3 {
                    log::warn!("data container too small for type");
                }
            }
            return Err(Error::ReadSdo(slave, idx));
        }
        Ok(slice)
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
        let wkc = self.0.sdo_write(
            u16::from(slave) + 1,
            index,
            subindex,
            access_complete,
            data,
            timeout,
        );

        if wkc <= 0 {
            let errs = self.ctx_errors();
            log::debug!("{:?}", errs);
            return Err(Error::WriteSdo(slave, idx));
        }
        Ok(())
    }

    fn ctx_errors(&mut self) -> Vec<ctx::Error> {
        let mut errors = vec![];
        while let Some(e) = self.0.pop_error() {
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
