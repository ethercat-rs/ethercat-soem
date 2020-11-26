use ethercat_types as ec;
use num_traits::cast::FromPrimitive;
use std::{
    ffi::{CStr, CString},
    mem,
    os::raw::{c_int, c_void},
    time::Duration,
};

mod context;
mod error;
mod slave;

use context::sys;
use slave::Slave;

pub use error::Error;

const EC_TIMEOUTRET: i32 = 2_000;

type Result<T> = std::result::Result<T, Error>;

pub struct Master(Box<context::Ctx>);

impl Master {
    pub fn try_new<S: Into<String>>(iface: S) -> Result<Self> {
        let mut master = Self(Box::new(context::Ctx::default()));
        master.init(iface.into())?;
        Ok(master)
    }

    #[doc(hidden)]
    /// Don't use this!
    pub fn ptr(&mut self) -> *mut context::Ctx {
        let reference: &mut context::Ctx = &mut *self.0;
        reference as *mut context::Ctx
    }

    #[doc(hidden)]
    /// Don't use this!
    pub unsafe fn from_ptr(ctx_ptr: *mut context::Ctx) -> Self {
        Master(Box::from_raw(ctx_ptr))
    }

    fn init(&mut self, iface: String) -> Result<()> {
        log::debug!("Initialise SOEM stack: bind socket to {}", iface);
        let iface = CString::new(iface).map_err(|_| Error::Iface)?;
        let init_res = unsafe { sys::ecx_init(self.ctx(), iface.as_ptr()) };
        if init_res <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::Init);
        }
        Ok(())
    }

    fn ctx(&mut self) -> &mut sys::ecx_context {
        self.0.ecx()
    }

    pub fn auto_config(&mut self) -> Result<()> {
        log::debug!("Find and auto-config slaves");
        let usetable = 0; // false
        let res = unsafe { sys::ecx_config_init(self.ctx(), usetable) };
        if res <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(match res {
                -1 => Error::NoFrame,
                -2 => Error::OtherFrame,
                _ => Error::NoSlaves,
            });
        }
        log::debug!("{} slaves found and configured", self.slave_count());
        let group = 0;
        let io_map_size = unsafe {
            sys::ecx_config_map_group(
                self.ctx(),
                self.0.io_map.as_mut_ptr() as *mut std::ffi::c_void,
                group,
            )
        };
        if io_map_size <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::CfgMapGroup);
        }
        let res = unsafe { sys::ecx_configdc(self.ctx()) };
        if res == 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::CfgDc);
        }
        Ok(())
    }

    pub fn request_states(&mut self, state: ec::AlState) -> Result<()> {
        log::debug!("wait for all slaves to reach {:?} state", state);
        for i in 0..self.slave_count() {
            self.0.slave_list[i].set_state(state);
            let wkc = unsafe { sys::ecx_writestate(self.ctx(), i as u16) };
            if wkc <= 0 {
                log::debug!("{:?}", self.errors());
                log::warn!("Could not set state {:?} for slave {}", state, i);
                return Err(Error::SetState);
            }
        }
        Ok(())
    }

    pub fn check_states(&mut self, state: ec::AlState) -> Result<()> {
        for i in 0..self.slave_count() {
            let res = unsafe {
                sys::ecx_statecheck(self.ctx(), i as u16, u8::from(state) as u16, 50_000)
            };
            if res == 0 {
                log::debug!("{:?}", self.errors());
                log::warn!("Could not check state {:?} for slave {}", state, i);
                return Err(Error::CheckState);
            }
        }
        Ok(())
    }

    pub fn slaves(&mut self) -> &mut [Slave] {
        let cnt = self.slave_count();
        &mut self.0.slave_list[1..=cnt]
    }

    pub fn states(&mut self) -> Result<Vec<ec::AlState>> {
        let lowest_state = unsafe { sys::ecx_readstate(self.ctx()) };
        if lowest_state <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::ReadStates);
        }
        // TODO: check 'ALstatuscode'
        let states = (1..=self.slave_count())
            .into_iter()
            .map(|i| self.slave_state(i))
            .collect::<Result<_>>()?;
        Ok(states)
    }

    fn slave_state(&self, slave: usize) -> Result<ec::AlState> {
        self.0.slave_list[slave].state()
    }

    pub fn send_processdata(&mut self) -> Result<()> {
        unsafe { sys::ecx_send_processdata(self.ctx()) };
        if self.is_err() {
            log::debug!("{:?}", self.errors());
            return Err(Error::SendProcessData);
        }
        Ok(())
    }

    pub fn recv_processdata(&mut self) -> Result<usize> {
        let wkc = unsafe { sys::ecx_receive_processdata(self.ctx(), EC_TIMEOUTRET) };
        if self.is_err() {
            log::debug!("{:?}", self.errors());
            return Err(Error::RecvProcessData);
        }
        Ok(wkc as usize)
    }

    pub fn group_outputs_wkc(&mut self, i: usize) -> Result<usize> {
        if i >= self.0.group_list.len() || i >= self.max_group() {
            if self.is_err() {
                log::debug!("{:?}", self.errors());
            }
            return Err(Error::GroupId);
        }
        Ok(self.0.group_list[i].outputsWKC as usize)
    }

    pub fn group_inputs_wkc(&mut self, i: usize) -> Result<usize> {
        if i >= self.0.group_list.len() || i >= self.max_group() {
            log::debug!("{:?}", self.errors());
            return Err(Error::GroupId);
        }
        Ok(self.0.group_list[i].inputsWKC as usize)
    }
    pub fn slave_count(&self) -> usize {
        self.0.slave_count() as usize
    }

    pub fn max_group(&self) -> usize {
        self.0.max_group()
    }

    pub fn dc_time(&self) -> i64 {
        *self.0.dc_time
    }

    fn read_od_desc(&mut self, item: u16, od_list: &mut sys::ec_ODlistt) -> Result<ec::SdoInfo> {
        let res = unsafe { sys::ecx_readODdescription(self.ctx(), item, &mut *od_list) };
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::ReadOdDesc(pos));
        }
        let i = item as usize;
        let idx = ec::Idx::from(od_list.Index[i]);
        let object_code = Some(od_list.ObjectCode[i]);
        let name = od_list.Name[i];
        let max_sub_idx = ec::SubIdx::from(od_list.MaxSub[i]);
        let name = c_array_to_string(name.as_ptr());
        let info = ec::SdoInfo {
            pos,
            idx,
            max_sub_idx,
            object_code,
            name,
        };
        Ok(info)
    }

    fn read_oe_list(
        &mut self,
        item: u16,
        od_list: &mut sys::ec_ODlistt,
    ) -> Result<sys::ec_OElistt> {
        let mut oe_list: sys::ec_OElistt = unsafe { mem::zeroed() };
        let res = unsafe { sys::ecx_readOE(self.ctx(), item, &mut *od_list, &mut oe_list) };
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::ReadOeList(pos));
        }
        Ok(oe_list)
    }

    pub fn read_od_list(
        &mut self,
        slave: ec::SlavePos,
    ) -> Result<Vec<(ec::SdoInfo, Vec<ec::SdoEntryInfo>)>> {
        let mut od_list: sys::ec_ODlistt = unsafe { mem::zeroed() };
        let res = unsafe { sys::ecx_readODlist(self.ctx(), u16::from(slave) + 1, &mut od_list) };

        if res <= 0 {
            log::debug!("{:?}", self.errors());
            return Err(Error::ReadOdList(slave));
        }
        log::debug!("CoE Object Description: found {} entries", od_list.Entries);
        let mut sdos = vec![];
        for i in 0..od_list.Entries {
            let sdo_info = self.read_od_desc(i, &mut od_list)?;
            let oe_list = self.read_oe_list(i, &mut od_list)?;

            let mut entries = vec![];

            for j in 0..=u8::from(sdo_info.max_sub_idx) as usize {
                if oe_list.DataType[j] > 0 && oe_list.BitLength[j] > 0 {
                    let dt = oe_list.DataType[j];
                    let data_type = ec::DataType::from_u16(dt).unwrap_or_else(|| {
                        log::warn!("Unknown DataType {}: use RAW as fallback", dt);
                        ec::DataType::Raw
                    });
                    let bit_len = oe_list.BitLength[j];
                    let access = access_from_u16(oe_list.ObjAccess[j]);
                    let description = c_array_to_string(oe_list.Name[j].as_ptr());
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
        let mut size = mem::size_of_val(target) as c_int;
        let index = u16::from(idx.idx);
        let subindex = u8::from(idx.sub_idx);
        let timeout = timeout.as_micros() as i32; //TODO: check overflow

        let wkc = unsafe {
            sys::ecx_SDOread(
                self.ctx(),
                u16::from(slave) + 1,
                index,
                subindex,
                if access_complete { 1 } else { 0 },
                &mut size,
                target.as_mut_ptr() as *mut c_void,
                timeout,
            )
        };
        if wkc <= 0 {
            let errs = self.errors();
            log::debug!("{:?}", errs);
            for e in errs {
                if e.e_type == context::ErrType::Packet && e.code == 3 {
                    log::warn!("data container too small for type");
                }
            }
            return Err(Error::ReadSdo(slave, idx));
        }
        Ok(&mut target[..size as usize])
    }

    pub fn write_sdo(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
        access_complete: bool,
        data: &[u8],
        timeout: Duration,
    ) -> Result<()> {
        let size = mem::size_of_val(data) as c_int;
        let index = u16::from(idx.idx);
        let subindex = u8::from(idx.sub_idx);
        let timeout = timeout.as_micros() as i32; //TODO: check overflow

        let wkc = unsafe {
            sys::ecx_SDOwrite(
                self.ctx(),
                u16::from(slave) + 1,
                index,
                subindex,
                if access_complete { 1 } else { 0 },
                size,
                data.as_ptr() as *mut c_void,
                timeout,
            )
        };

        if wkc <= 0 {
            let errs = self.errors();
            log::debug!("{:?}", errs);
            return Err(Error::WriteSdo(slave, idx));
        }
        Ok(())
    }

    fn is_err(&mut self) -> bool {
        self.0.is_err()
    }

    fn errors(&mut self) -> Vec<context::EcError> {
        self.0.errors()
    }
}

fn c_array_to_string(data: *const i8) -> String {
    unsafe { CStr::from_ptr(data).to_string_lossy().into_owned() }
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
