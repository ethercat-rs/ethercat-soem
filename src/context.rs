pub use ethercat_soem_sys as sys;
use std::mem::zeroed;

const EC_MAXGROUP: usize = 2;
const EC_MAXSLAVE: usize = 200;

/// Size of EEPROM bitmap cache
const EC_MAXEEPBITMAP: usize = 128;

/// Size of EEPROM cache buffer
const EC_MAXEEPBUF: usize = EC_MAXEEPBITMAP << 5;

/// SOEM context wrapper
pub struct Ctx {
    #[allow(dead_code)]
    port: Box<sys::ecx_portt>,
    pub slave_list: Box<[sys::ec_slave; EC_MAXSLAVE]>,
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
    pub ecx_ctx: sys::ecx_context,

    /// I/O map
    pub io_map: [u8; 4096],
}

impl Ctx {
    pub fn new() -> Self {
        let mut port = Box::new(sys::ecx_portt {
            _bindgen_opaque_blob: [0; 6502],
        });
        let mut slave_list: Box<[sys::ec_slave; EC_MAXSLAVE]> = Box::new(unsafe { zeroed() });
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

    pub fn slave_count(&self) -> i32 {
        *self.slave_count
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn context_wrapper() {
        let mut wrapper = Ctx::new();
        wrapper.port._bindgen_opaque_blob[3] = 5;
        assert_eq!(
            unsafe { (*wrapper.ecx_ctx.port)._bindgen_opaque_blob[3] },
            5
        );
        assert_eq!(wrapper.slave_list.len(), 200);
        assert_eq!(wrapper.slave_list[7].ALstatuscode, 0);
        wrapper.slave_list[7].ALstatuscode = 33;
        assert_eq!(
            unsafe {
                std::slice::from_raw_parts_mut(wrapper.ecx_ctx.slavelist, 200)[7].ALstatuscode
            },
            33
        );
    }
}
