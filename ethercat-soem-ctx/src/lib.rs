//! SOEM low level context

#![cfg_attr(not(debug_assertions), deny(warnings))]
#![deny(rust_2018_idioms)]
#![deny(rust_2021_compatibility)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![deny(clippy::explicit_deref_methods)]
#![deny(clippy::explicit_into_iter_loop)]
#![deny(clippy::explicit_iter_loop)]
#![deny(clippy::must_use_candidate)]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

use ethercat_soem_sys as sys;
use std::{
    ffi::{CStr, CString},
    mem::{self, zeroed},
    os::raw::c_void,
    time::Duration,
};

mod error;
mod group;
mod od_list;
mod oe_list;
mod slave;
mod sm;

pub use crate::{error::*, group::*, od_list::*, oe_list::*, slave::*};

const EC_MAX_GROUP: usize = 2;
const EC_MAX_SLAVE: usize = 200;

/// Size of EEPROM bitmap cache
const EC_MAX_EEP_BITMAP: usize = 128;

/// Size of EEPROM cache buffer
const EC_MAX_EEP_BUF: usize = EC_MAX_EEP_BITMAP << 5;

/// SOEM `ecx_context` wrapper
#[allow(missing_debug_implementations)]
pub struct Ctx {
    #[allow(dead_code)]
    port: Box<sys::ecx_portt>,
    slave_list: Box<[Slave; EC_MAX_SLAVE]>,
    slave_count: Box<i32>,
    group_list: Box<[Group; EC_MAX_GROUP]>,
    #[allow(dead_code)]
    esi_buf: Box<[u8; EC_MAX_EEP_BUF]>,
    #[allow(dead_code)]
    esi_map: Box<[u32; EC_MAX_EEP_BITMAP]>,
    #[allow(dead_code)]
    e_list: Box<[u32; 456]>,
    #[allow(dead_code)]
    idx_stack: Box<[u64; 27]>,
    #[allow(dead_code)]
    ecat_error: Box<u8>,
    dc_time: Box<i64>,
    #[allow(dead_code)]
    sm_comm_type: Box<[u8; 10]>,
    #[allow(dead_code)]
    pdo_assign: Box<[u8; 514]>,
    #[allow(dead_code)]
    pdo_desc: Box<[u8; 1026]>,
    #[allow(dead_code)]
    eep_sm: Box<[u16; 6]>,
    #[allow(dead_code)]
    eep_fmmu: Box<[u16; 4]>,

    /// The original context
    ecx_ctx: sys::ecx_context,

    /// I/O map
    pub io_map: [u8; 4096],
}

impl Default for Ctx {
    fn default() -> Self {
        let mut port = Box::new(sys::ecx_portt {
            _bindgen_opaque_blob: [0; 6502],
        });
        let mut slave_list: Box<[Slave; EC_MAX_SLAVE]> = Box::new(unsafe { zeroed() });
        let mut slave_count = Box::new(0);
        let mut group_list: Box<[Group; EC_MAX_GROUP]> = Box::new(unsafe { zeroed() });
        let mut esi_buf: Box<[u8; EC_MAX_EEP_BUF]> = Box::new([0; EC_MAX_EEP_BUF]);
        let mut esi_map: Box<[u32; EC_MAX_EEP_BITMAP]> = Box::new([0; EC_MAX_EEP_BITMAP]);
        let mut e_list: Box<[u32; 456]> = Box::new([0; 456]);
        let mut idx_stack: Box<[u64; 27]> = Box::new([0; 27]);
        let mut ecat_error: Box<u8> = Box::new(0);
        let mut dc_time = Box::new(0);
        let mut sm_comm_type: Box<[u8; 10]> = Box::new([0; 10]);
        let mut pdo_assign: Box<[u8; 514]> = Box::new([0; 514]);
        let mut pdo_desc: Box<[u8; 1026]> = Box::new([0; 1026]);
        let mut eep_sm: Box<[u16; 6]> = Box::new([0; 6]);
        let mut eep_fmmu: Box<[u16; 4]> = Box::new([0; 4]);
        let io_map = [0; 4096];

        // The original context
        let ecx_ctx = sys::ecx_context {
            port: &mut *port,
            slavelist: slave_list.as_mut_ptr() as *mut sys::ec_slave,
            slavecount: &mut *slave_count,
            maxslave: EC_MAX_SLAVE as i32,
            grouplist: group_list.as_mut_ptr() as *mut sys::ec_group,
            maxgroup: EC_MAX_GROUP as i32,
            esibuf: esi_buf.as_mut_ptr(),
            esimap: esi_map.as_mut_ptr(),
            esislave: 0,
            elist: &mut *e_list,
            idxstack: &mut *idx_stack,
            ecaterror: &mut *ecat_error,
            DCtime: &mut *dc_time,
            SMcommtype: &mut *sm_comm_type,
            PDOassign: &mut *pdo_assign,
            PDOdesc: &mut *pdo_desc,
            eepSM: &mut *eep_sm,
            eepFMMU: &mut *eep_fmmu,
            FOEhook: None,
            EOEhook: None,
            manualstatechange: 0,
            userdata: std::ptr::null_mut(),
        };

        Self {
            port,
            slave_list,
            slave_count,
            group_list,
            esi_buf,
            esi_map,
            e_list,
            idx_stack,
            ecat_error,
            dc_time,
            sm_comm_type,
            pdo_assign,
            pdo_desc,
            eep_sm,
            eep_fmmu,
            ecx_ctx,
            io_map,
        }
    }
}

impl Ctx {
    /// Initialise lib in single NIC mode.
    ///
    /// Return > 0 if OK.
    pub fn init(&mut self, iface: CString) -> i32 {
        unsafe { sys::ecx_init(&mut self.ecx_ctx, iface.as_ptr()) }
    }
    pub fn config_init(&mut self, use_table: bool) -> i32 {
        unsafe { sys::ecx_config_init(&mut self.ecx_ctx, if use_table { 1 } else { 0 }) }
    }
    pub fn config_map_group(&mut self, group: u8) -> i32 {
        unsafe {
            sys::ecx_config_map_group(
                &mut self.ecx_ctx,
                self.io_map.as_mut_ptr() as *mut std::ffi::c_void,
                group,
            )
        }
    }
    pub fn config_dc(&mut self) -> u8 {
        unsafe { sys::ecx_configdc(&mut self.ecx_ctx) }
    }
    pub const fn slave_count(&self) -> usize {
        *self.slave_count as usize
    }
    pub fn slaves(&self) -> &[Slave; EC_MAX_SLAVE] {
        &self.slave_list
    }
    pub fn slaves_mut(&mut self) -> &mut [Slave; EC_MAX_SLAVE] {
        &mut self.slave_list
    }
    pub const fn groups(&self) -> &[Group; EC_MAX_GROUP] {
        &self.group_list
    }
    /// Write slave state, if slave = 0 then write to all slaves.
    ///
    /// The function does not check if the actual state is changed.
    /// It returns Workcounter or `EC_NOFRAME` (= `-1`),
    pub fn write_state(&mut self, slave: u16) -> i32 {
        unsafe { sys::ecx_writestate(&mut self.ecx_ctx, slave) }
    }
    /// Check actual slave state.
    ///
    /// This is a blocking function.
    /// To refresh the state of all slaves `read_state()` should be called.
    ///
    /// Parameter `slave` = Slave number, 0 = all slaves (only the "slavelist[0].state" is refreshed).
    ///
    /// It returns requested state, or found state after timeout.
    pub fn state_check(&mut self, slave: u16, state: u16, timeout: Duration) -> u16 {
        unsafe { sys::ecx_statecheck(&mut self.ecx_ctx, slave, state, timeout.as_micros() as i32) }
    }
    /// Read all slave states in ec_slave.
    ///
    /// It returns the lowest state found,
    pub fn read_state(&mut self) -> i32 {
        unsafe { sys::ecx_readstate(&mut self.ecx_ctx) }
    }
    pub fn send_processdata(&mut self) -> i32 {
        unsafe { sys::ecx_send_processdata(&mut self.ecx_ctx) }
    }
    pub fn receive_processdata(&mut self, timeout: Duration) -> i32 {
        unsafe { sys::ecx_receive_processdata(&mut self.ecx_ctx, timeout.as_micros() as i32) }
    }
    pub fn read_od_list(&mut self, slave: u16, od_list: &mut OdList) -> i32 {
        unsafe { sys::ecx_readODlist(&mut self.ecx_ctx, slave, &mut od_list.0) }
    }
    pub fn read_od_description(&mut self, item: u16, od_list: &mut OdList) -> i32 {
        unsafe { sys::ecx_readODdescription(&mut self.ecx_ctx, item, &mut od_list.0) }
    }
    pub fn read_oe(&mut self, item: u16, od_list: &mut OdList, oe_list: &mut OeList) -> i32 {
        unsafe { sys::ecx_readOE(&mut self.ecx_ctx, item, &mut od_list.0, &mut oe_list.0) }
    }
    pub fn sdo_read<'t>(
        &mut self,
        slave: u16,
        idx: u16,
        sub_idx: u8,
        access_complete: bool,
        target: &'t mut [u8],
        timeout: Duration,
    ) -> (i32, &'t mut [u8]) {
        let mut size = mem::size_of_val(target) as i32;
        let timeout = timeout.as_micros() as i32; //TODO: check overflow
        let wkc = unsafe {
            sys::ecx_SDOread(
                &mut self.ecx_ctx,
                slave,
                idx,
                sub_idx,
                if access_complete { 1 } else { 0 },
                &mut size,
                target.as_mut_ptr() as *mut c_void,
                timeout,
            )
        };
        if wkc <= 0 {
            (wkc, target)
        } else {
            (wkc, &mut target[..size as usize])
        }
    }
    pub fn sdo_write(
        &mut self,
        slave: u16,
        idx: u16,
        sub_idx: u8,
        access_complete: bool,
        data: &[u8],
        timeout: Duration,
    ) -> i32 {
        let size = mem::size_of_val(data) as i32;
        let timeout = timeout.as_micros() as i32; //TODO: check overflow
        unsafe {
            sys::ecx_SDOwrite(
                &mut self.ecx_ctx,
                slave,
                idx,
                sub_idx,
                if access_complete { 1 } else { 0 },
                size,
                data.as_ptr() as *mut c_void,
                timeout,
            )
        }
    }
    pub const fn max_group(&self) -> i32 {
        self.ecx_ctx.maxgroup
    }
    pub fn is_err(&mut self) -> bool {
        unsafe { sys::ecx_iserror(&mut self.ecx_ctx) != 0 }
    }
    pub fn pop_error(&mut self) -> Option<Error> {
        let mut ec: sys::ec_errort = unsafe { zeroed() };
        if unsafe { sys::ecx_poperror(&mut self.ecx_ctx, &mut ec) } != 0 {
            Some(Error::from(ec))
        } else {
            None
        }
    }
    pub const fn dc_time(&self) -> i64 {
        *self.dc_time
    }
}

fn c_array_to_string(data: *const i8) -> String {
    unsafe { CStr::from_ptr(data).to_string_lossy().into_owned() }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn context_wrapper() {
        let mut wrapper = Ctx::default();
        wrapper.port._bindgen_opaque_blob[3] = 5;
        assert_eq!(
            unsafe { (*wrapper.ecx_ctx.port)._bindgen_opaque_blob[3] },
            5
        );
        assert_eq!(wrapper.slave_list.len(), 200);
        assert_eq!(wrapper.slave_list[7].0.ALstatuscode, 0);
        wrapper.slave_list[7].0.ALstatuscode = 33;
        assert_eq!(
            unsafe {
                std::slice::from_raw_parts_mut(wrapper.ecx_ctx.slavelist, 200)[7].ALstatuscode
            },
            33
        );
    }
}
