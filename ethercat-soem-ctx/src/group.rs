use ethercat_soem_sys as sys;
use std::{fmt, mem};

/// SOEM `ec_group` wrapper
#[repr(C)]
pub struct Group(pub(crate) sys::ec_group);

impl Default for Group {
    fn default() -> Self {
        Group(unsafe { mem::zeroed() })
    }
}

impl Group {
    // TODO:
    // logical start address for this group
    //pub logstartaddr: uint32,

    // TODO:
    // output bytes, if Obits < 8 then Obytes = 0
    //pub Obytes: uint32,

    // TODO:
    // output pointer in IOmap buffer
    //pub outputs: *mut uint8,

    // TODO:
    // input bytes, if Ibits < 8 then Ibytes = 0
    //pub Ibytes: uint32,

    // TODO:
    // input pointer in IOmap buffer
    //pub inputs: *mut uint8,

    // TODO:
    // has DC capability
    // pub hasdc: boolean,

    // TODO:
    // next DC slave
    // pub DCnext: uint16,

    // TODO:
    // E-bus current
    // pub Ebuscurrent: int16,

    // TODO:
    // if >0 block use of LRW in processdata
    // pub blockLRW: uint8,

    // TODO:
    // IO segments used
    // pub nsegments: uint16,

    // TODO:
    // 1st input segment
    // pub Isegment: uint16,

    // TODO:
    // Offset in input segment
    // pub Ioffset: uint16,

    /// Expected workcounter outputs
    pub const fn outputs_wkc(&self) -> u16 {
        self.0.outputsWKC
    }
    /// Expected workcounter inputs"]
    pub const fn inputs_wkc(&self) -> u16 {
        self.0.inputsWKC
    }

    // TODO:
    // check slave states
    // pub docheckstate: boolean,

    // TODO:
    // IO segmentation list. Datagrams must not break SM in two.
    // pub IOsegment: [uint32; 64usize],
}

impl fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: add missing fields
        f.debug_struct("Group")
            .field("outputs_wkc", &self.outputs_wkc())
            .field("inputs_wkc", &self.inputs_wkc())
            .finish()
    }
}
