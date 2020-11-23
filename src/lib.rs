use ethercat_types as ec;
use num_traits::cast::FromPrimitive;
use std::{
    convert::TryFrom,
    ffi::{CStr, CString},
    mem,
    os::raw::{c_int, c_void},
    time::Duration,
};
use thiserror::Error;

mod context;
use context::sys;

const EC_TIMEOUTRET: i32 = 2_000;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not init EtherCAT master")]
    Init,
    #[error("Invalid network interface")]
    Iface,
    #[error("No slaves found")]
    NoSlaves,
    #[error("Could not configure map group")]
    CfgMapGroup,
    #[error("Could not configure DC")]
    CfgDc,
    #[error("Could not set requested state")]
    SetState,
    #[error("Could not check state")]
    CheckState,
    #[error("Could not read states")]
    ReadStates,
    #[error("Could not send process data")]
    SendProcessData,
    #[error("Invalid AL state")]
    AlState,
    #[error("Invalid group ID")]
    GroupId,
    #[error("Could not read OD list of {0:?}")]
    ReadOdList(ec::SlavePos),
    #[error("Could not read OD description of {0:?}")]
    ReadOdDesc(ec::SdoPos),
    #[error("Could not read OE list of {0:?}")]
    ReadOeList(ec::SdoPos),
    #[error("Could not read {1:?} of {0:?}")]
    ReadSdo(ec::SlavePos, ec::SdoIdx),
}

type Result<T> = std::result::Result<T, Error>;

pub struct Master {
    #[allow(dead_code)]
    iface: String,
    ctx: context::Ctx,
}

impl Master {
    pub fn new<S: Into<String>>(iface: S) -> Result<Self> {
        let mut master = Self {
            iface: iface.into(),
            ctx: context::Ctx::new(),
        };
        master.init()?;
        Ok(master)
    }

    fn init(&mut self) -> Result<()> {
        log::debug!("Initialise SOEM stack: bind socket to {}", self.iface);
        let iface = CString::new(self.iface.clone()).map_err(|_| Error::Iface)?;
        let init_res = unsafe { sys::ecx_init(&mut self.ctx.ecx_ctx, iface.as_ptr()) };
        if init_res <= 0 {
            return Err(Error::Init);
        }
        Ok(())
    }

    pub fn auto_config(&mut self) -> Result<()> {
        log::debug!("Find and auto-config slaves");
        let usetable = 0; // false
        let cfg_init_res = unsafe { sys::ecx_config_init(&mut self.ctx.ecx_ctx, usetable) };
        if cfg_init_res <= 0 {
            return Err(Error::NoSlaves);
        }
        log::debug!("{} slaves found and configured", self.slave_count());
        let group = 0;
        let cfg_map_res = unsafe {
            sys::ecx_config_map_group(
                &mut self.ctx.ecx_ctx,
                self.ctx.io_map.as_mut_ptr() as *mut std::ffi::c_void,
                group,
            )
        };
        if cfg_map_res <= 0 {
            return Err(Error::CfgMapGroup);
        }
        let cfg_dc_res = unsafe { sys::ecx_configdc(&mut self.ctx.ecx_ctx) };
        if cfg_dc_res == 0 {
            return Err(Error::CfgDc);
        }
        Ok(())
    }

    pub fn request_states(&mut self, state: ec::AlState) -> Result<()> {
        log::debug!("wait for all slaves to reach {:?} state", state);
        for i in 0..self.slave_count() {
            self.ctx.slave_list[i].state = u8::from(state) as u16;
            let res = unsafe { sys::ecx_writestate(&mut self.ctx.ecx_ctx, i as u16) };
            if res <= 0 {
                log::warn!("Could not set state {:?} for slave {}", state, i);
                return Err(Error::SetState);
            }
        }
        Ok(())
    }

    pub fn check_states(&mut self, state: ec::AlState) -> Result<()> {
        for i in 0..self.slave_count() {
            let res = unsafe {
                sys::ecx_statecheck(
                    &mut self.ctx.ecx_ctx,
                    i as u16,
                    u8::from(state) as u16,
                    50_000,
                )
            };
            if res == 0 {
                log::warn!("Could not check state {:?} for slave {}", state, i);
                return Err(Error::CheckState);
            }
        }
        Ok(())
    }

    pub fn states(&mut self) -> Result<Vec<ec::AlState>> {
        let res = unsafe { sys::ecx_readstate(&mut self.ctx.ecx_ctx) };
        if res <= 0 {
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
        let s = self.ctx.slave_list[slave].state;
        ec::AlState::try_from(s as u8).map_err(|_| Error::AlState)
    }

    pub fn send_processdata(&mut self) -> Result<()> {
        let res = unsafe { sys::ecx_send_processdata(&mut self.ctx.ecx_ctx) };
        if res <= 0 {
            return Err(Error::SendProcessData);
        }
        Ok(())
    }

    pub fn recv_processdata(&mut self) -> Result<usize> {
        let wkc = unsafe { sys::ecx_receive_processdata(&mut self.ctx.ecx_ctx, EC_TIMEOUTRET) };
        if wkc <= 0 {
            return Err(Error::SendProcessData);
        }
        Ok(wkc as usize)
    }

    pub fn group_outputs_wkc(&self, i: usize) -> Result<usize> {
        if i >= self.ctx.group_list.len() || i >= self.max_group() {
            return Err(Error::GroupId);
        }
        Ok(self.ctx.group_list[i].outputsWKC as usize)
    }

    pub fn group_inputs_wkc(&self, i: usize) -> Result<usize> {
        if i >= self.ctx.group_list.len() || i >= self.max_group() {
            return Err(Error::GroupId);
        }
        Ok(self.ctx.group_list[i].inputsWKC as usize)
    }
    pub fn slave_count(&self) -> usize {
        self.ctx.slave_count() as usize
    }

    pub fn max_group(&self) -> usize {
        self.ctx.ecx_ctx.maxgroup as usize
    }

    pub fn dc_time(&self) -> i64 {
        *self.ctx.dc_time
    }

    fn read_od_desc(&mut self, item: u16, od_list: &mut sys::ec_ODlistt) -> Result<ec::SdoInfo> {
        let res = unsafe { sys::ecx_readODdescription(&mut self.ctx.ecx_ctx, item, &mut *od_list) };
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
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
        let res =
            unsafe { sys::ecx_readOE(&mut self.ctx.ecx_ctx, item, &mut *od_list, &mut oe_list) };
        let pos = ec::SdoPos::from(item);
        if res <= 0 {
            return Err(Error::ReadOeList(pos));
        }
        Ok(oe_list)
    }

    pub fn read_od_list(
        &mut self,
        slave: ec::SlavePos,
    ) -> Result<Vec<(ec::SdoInfo, Vec<ec::SdoEntryInfo>)>> {
        let mut od_list: sys::ec_ODlistt = unsafe { mem::zeroed() };
        let res = unsafe {
            sys::ecx_readODlist(&mut self.ctx.ecx_ctx, u16::from(slave) + 1, &mut od_list)
        };

        if res <= 0 {
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
                    let access = ec::SdoEntryAccess {
                        pre_op: ec::Access::Unknown,  // TODO: useoe_list.ObjAccess[j]
                        safe_op: ec::Access::Unknown, // TODO: useoe_list.ObjAccess[j]
                        op: ec::Access::Unknown,      // TODO: useoe_list.ObjAccess[j]
                    };
                    let description = c_array_to_string(oe_list.Name[j].as_ptr());

                    let entry_info = ec::SdoEntryInfo {
                        data_type,
                        bit_len,
                        access,
                        description,
                    };
                    entries.push(entry_info);
                }
            }
            sdos.push((sdo_info, entries));
        }
        Ok(sdos)
    }

    pub fn read_sdo<T: Sized>(
        &mut self,
        slave: ec::SlavePos,
        idx: ec::SdoIdx,
        access_complete: bool,
        timeout: Duration,
    ) -> Result<T> {
        let mut value: T = unsafe { mem::zeroed() };
        let value_ptr: *mut T = &mut value;
        let mut size = mem::size_of_val(&value) as c_int;

        let index = u16::from(idx.idx);
        let subindex = u8::from(idx.sub_idx);
        let timeout = timeout.as_micros() as i32; //TODO: check overflow

        let res = unsafe {
            sys::ecx_SDOread(
                &mut self.ctx.ecx_ctx,
                u16::from(slave) + 1,
                index,
                subindex,
                if access_complete { 1 } else { 0 },
                &mut size,
                value_ptr as *mut c_void,
                timeout,
            )
        };
        if res <= 0 {
            return Err(Error::ReadSdo(slave, idx));
        }
        Ok(value)
    }
}

fn c_array_to_string(data: *const i8) -> String {
    unsafe { CStr::from_ptr(data).to_string_lossy().into_owned() }
}
