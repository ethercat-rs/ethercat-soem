use crate::slave::Slave;
pub use ethercat_soem_sys as sys;
use std::{ffi::CStr, mem::zeroed};

const EC_MAXGROUP: usize = 2;
const EC_MAXSLAVE: usize = 200;

/// Size of EEPROM bitmap cache
const EC_MAXEEPBITMAP: usize = 128;

/// Size of EEPROM cache buffer
const EC_MAXEEPBUF: usize = EC_MAXEEPBITMAP << 5;

#[derive(Debug)]
pub struct EcError {
    pub e_type: ErrType,
    pub code: i32,
    pub msg: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrType {
    Emergency,
    Packet,
    Mbx,
    Sdo,
    SdoInfo,
    Soe,
    Foe,
    FoePacketNumber,
    FoeFileNotFound,
    FoeBufTooSmall,
    EoeInvalidRxData,
    Unknown(u32),
}

impl From<u32> for ErrType {
    fn from(t: u32) -> Self {
        match t {
            sys::ec_err_type_EC_ERR_TYPE_EMERGENCY => Self::Emergency,
            sys::ec_err_type_EC_ERR_TYPE_PACKET_ERROR => Self::Packet,
            sys::ec_err_type_EC_ERR_TYPE_MBX_ERROR => Self::Mbx,
            sys::ec_err_type_EC_ERR_TYPE_SDO_ERROR => Self::Sdo,
            sys::ec_err_type_EC_ERR_TYPE_SDOINFO_ERROR => Self::SdoInfo,
            sys::ec_err_type_EC_ERR_TYPE_SOE_ERROR => Self::Soe,
            sys::ec_err_type_EC_ERR_TYPE_FOE_ERROR => Self::Foe,
            sys::ec_err_type_EC_ERR_TYPE_FOE_PACKETNUMBER => Self::FoePacketNumber,
            sys::ec_err_type_EC_ERR_TYPE_FOE_FILE_NOTFOUND => Self::FoeFileNotFound,
            sys::ec_err_type_EC_ERR_TYPE_FOE_BUF2SMALL => Self::FoeBufTooSmall,
            sys::ec_err_type_EC_ERR_TYPE_EOE_INVALID_RX_DATA => Self::EoeInvalidRxData,

            _ => Self::Unknown(t),
        }
    }
}

impl From<sys::ec_errort> for EcError {
    fn from(e: sys::ec_errort) -> Self {
        let e_type = ErrType::from(e.Etype);
        let code = unsafe { e.__bindgen_anon_1.AbortCode };
        let msg = unsafe {
            let err_string = sys::ecx_err2string(e);
            CStr::from_ptr(err_string).to_string_lossy().into_owned()
        };
        EcError { e_type, code, msg }
    }
}

/// SOEM context wrapper
pub struct Ctx {
    #[allow(dead_code)]
    port: Box<sys::ecx_portt>,
    pub slave_list: Box<[Slave; EC_MAXSLAVE]>,
    #[allow(dead_code)]
    slave_count: Box<i32>,
    pub group_list: Box<[sys::ec_group; EC_MAXGROUP]>,
    #[allow(dead_code)]
    esi_buf: Box<[u8; EC_MAXEEPBUF]>,
    #[allow(dead_code)]
    esi_map: Box<[u32; EC_MAXEEPBITMAP]>,
    #[allow(dead_code)]
    e_list: Box<[u32; 456]>,
    #[allow(dead_code)]
    idx_stack: Box<[u64; 27]>,
    #[allow(dead_code)]
    ecat_error: Box<u8>,
    #[allow(dead_code)]
    pub dc_time: Box<i64>,
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
        let mut slave_list: Box<[Slave; EC_MAXSLAVE]> = Box::new(unsafe { zeroed() });
        let mut slave_count = Box::new(0);
        let mut group_list: Box<[sys::ec_group; EC_MAXGROUP]> = Box::new(unsafe { zeroed() });
        let mut esi_buf: Box<[u8; EC_MAXEEPBUF]> = Box::new([0; EC_MAXEEPBUF]);
        let mut esi_map: Box<[u32; EC_MAXEEPBITMAP]> = Box::new([0; EC_MAXEEPBITMAP]);
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
            maxslave: EC_MAXSLAVE as i32,
            grouplist: group_list.as_mut_ptr() as *mut sys::ec_group,
            maxgroup: EC_MAXGROUP as i32,
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
    pub fn slave_count(&self) -> i32 {
        *self.slave_count
    }

    pub fn max_group(&self) -> usize {
        self.ecx_ctx.maxgroup as usize
    }

    pub fn is_err(&mut self) -> bool {
        unsafe { sys::ecx_iserror(&mut self.ecx_ctx) != 0 }
    }

    pub fn pop_error(&mut self) -> Option<EcError> {
        let mut ec: sys::ec_errort = unsafe { zeroed() };
        if unsafe { sys::ecx_poperror(&mut self.ecx_ctx, &mut ec) } != 0 {
            Some(EcError::from(ec))
        } else {
            None
        }
    }

    pub fn errors(&mut self) -> Vec<EcError> {
        let mut errors = vec![];
        while let Some(e) = self.pop_error() {
            errors.push(e);
        }
        errors
    }

    pub fn ecx(&mut self) -> &mut sys::ecx_context {
        &mut self.ecx_ctx
    }
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
