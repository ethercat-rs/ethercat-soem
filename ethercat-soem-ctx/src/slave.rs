use ethercat_soem_sys as sys;
use std::{fmt, mem, slice, time::Duration};

/// SOEM `ec_slave` wrapper
#[repr(C)]
pub struct Slave(pub(crate) sys::ec_slave);

impl Default for Slave {
    fn default() -> Self {
        Self(unsafe { mem::zeroed() })
    }
}

impl Slave {
    /// State of slave
    pub const fn state(&self) -> u16 {
        self.0.state
    }
    /// Set state slave
    pub fn set_state(&mut self, s: u16) {
        self.0.state = s;
    }
    /// AL status code
    pub const fn al_status_code(&self) -> u16 {
        self.0.ALstatuscode
    }
    /// Configured address
    pub const fn config_addr(&self) -> u16 {
        self.0.configadr
    }
    /// Alias address
    pub const fn alias_addr(&self) -> u16 {
        self.0.aliasadr
    }
    /// Manufacturer from EEprom
    pub const fn eep_man(&self) -> u32 {
        self.0.eep_man
    }
    /// ID from EEprom
    pub const fn eep_id(&self) -> u32 {
        self.0.eep_id
    }
    /// Revision from EEprom
    pub const fn eep_rev(&self) -> u32 {
        self.0.eep_rev
    }

    // TODO:
    // Interface type
    // Itype: uint16

    // TODO:
    // Device type
    // Dtype: uint16

    /// Output bits
    pub const fn output_bits(&self) -> u16 {
        self.0.Obits
    }
    /// Output bytes
    ///
    /// if output_bits < 8 then output_bytes = 0
    pub const fn output_bytes(&self) -> u32 {
        self.0.Obytes
    }
    /// Imptable access to output data
    pub fn outputs(&self) -> &[u8] {
        let size = (if self.output_bytes() == 0 && self.output_bits() > 0 {
            1
        } else {
            self.output_bytes()
        }) as usize;
        unsafe { slice::from_raw_parts(self.0.outputs, size) }
    }
    /// Mutable access to output data
    pub fn outputs_mut(&mut self) -> &mut [u8] {
        let size = (if self.output_bytes() == 0 && self.output_bits() > 0 {
            1
        } else {
            self.output_bytes()
        }) as usize;
        unsafe { slice::from_raw_parts_mut(self.0.outputs, size) }
    }

    // TODO:
    // startbit in first output byte
    // Ostartbit: uint8

    /// Input bits
    pub const fn input_bits(&self) -> u16 {
        self.0.Ibits
    }
    /// Input bytes
    ///
    /// if input_bits < 8 then input_bytes = 0
    pub const fn input_bytes(&self) -> u32 {
        self.0.Ibytes
    }
    /// Inputs
    pub fn inputs(&self) -> &[u8] {
        let size = (if self.input_bytes() == 0 && self.input_bits() > 0 {
            1
        } else {
            self.input_bytes()
        }) as usize;
        unsafe { slice::from_raw_parts_mut(self.0.inputs, size) }
    }

    // TODO:
    // startbit in first input byte
    // Istartbit: uint8

    // TODO:
    // SM structure
    // SM: [ec_smt; 8]

    // TODO:
    // SM type 0=unused 1=MbxWr 2=MbxRd 3=Outputs 4=Inputs
    // SMtype: [uint8; 8]

    // TODO:
    // FMMU structure
    // FMMU: [ec_fmmut; 4]

    // TODO:
    // FMMU0 function
    // FMMU0func: uint8

    // TODO:
    // FMMU1 function
    // FMMU1func: uint8

    // TODO:
    // FMMU2 function
    // FMMU2func: uint8

    // TODO:
    // FMMU3 function
    // FMMU3func: uint8

    // TODO:
    // length of write mailbox in bytes, if no mailbox then 0
    // mbx_l: uint16

    // TODO:
    // mailbox write offset
    // mbx_wo: uint16

    // TODO:
    // length of read mailbox in bytes
    // mbx_rl: uint16

    // TODO:
    // mailbox read offset
    // mbx_ro: uint16

    // TODO:
    // mailbox supported protocols
    // mbx_proto: uint16

    // TODO:
    // Counter value of mailbox link layer protocol 1..7
    // mbx_cnt: uint8

    /// Has DC capability
    pub const fn has_dc(&self) -> bool {
        self.0.hasdc != 0
    }

    // TODO:
    // Physical type; Ebus, EtherNet combinations
    // ptype: uint8

    // TODO:
    // topology: 1 to 3 links
    // topology: uint8

    // TODO:
    // active ports bitmap : ....3210 , set if respective port is active
    // activeports: uint8

    // TODO:
    // consumed ports bitmap : ....3210, used for internal delay measurement
    // consumedports: uint8

    // TODO:
    // slave number for parent, 0=master
    // parent: uint16

    /// Port number on parent this slave is connected to
    pub const fn parent_port(&self) -> u8 {
        self.0.parentport
    }

    // TODO:
    // port number on this slave the parent is connected to
    // entryport: uint8

    // TODO:
    // DC receivetimes on port A
    // DCrtA: int32

    // TODO:
    // DC receivetimes on port B
    // DCrtB: int32

    // TODO:
    // DC receivetimes on port C
    // DCrtC: int32

    // TODO:
    // DC receivetimes on port D
    // DCrtD: int32

    /// Propagation delay
    pub const fn propagation_delay(&self) -> Duration {
        Duration::from_nanos(self.0.pdelay as u64)
    }

    // TODO:
    // next DC slave
    // DCnext: uint16

    // TODO:
    // previous DC slave
    // DCprevious: uint16

    // TODO:
    // DC cycle time in ns
    // DCcycle: int32

    // TODO:
    // DC shift from clock modulus boundary
    // DCshift: int32

    // TODO:
    // DC sync activation, 0=off, 1=on
    // DCactive: uint8

    // TODO:
    // link to config table
    // configindex: uint16

    // TODO:
    // link to SII config
    // SIIindex: uint16

    // TODO:
    // 1 = 8 bytes per read, 0 = 4 bytes per read
    // eep_8byte: uint8

    // TODO:
    // 0 = eeprom to master , 1 = eeprom to PDI
    // eep_pdi: uint8

    // TODO:
    // CoE details
    // CoEdetails: uint8

    // TODO:
    // FoE details
    // FoEdetails: uint8

    // TODO:
    // EoE details
    // EoEdetails: uint8

    // TODO:
    // SoE details
    // SoEdetails: uint8

    // TODO:
    // E-bus current
    // Ebuscurrent: int16

    // TODO:
    // if >0 block use of LRW in processdata
    // blockLRW: uint8

    /// Group
    pub const fn group(&self) -> u8 {
        self.0.group
    }

    // TODO:
    // first unused FMMU
    // FMMUunused: uint8

    // TODO:
    // Boolean for tracking whether the slave is (not) responding,
    // not used/set by the SOEM library
    // islost: boolean

    // TODO:
    // registered configuration function PO->SO, (DEPRECATED)
    // PO2SOconfig: Option<unsafe extern "C" fn(slave: uint16) -> c_int>

    // TODO:
    // registered configuration function PO->SO
    // PO2SOconfigx:
    // Option<unsafe extern "C" fn(context: *mut ecx_contextt, slave: uint16) -> c_int>

    /// Readable name
    pub fn name(&self) -> String {
        super::c_array_to_string(self.0.name.as_ptr())
    }
}

impl fmt::Debug for Slave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: add missing fields
        f.debug_struct("Slave")
            .field("state", &self.state())
            .field("al_status_code", &self.al_status_code())
            .field("config_addr", &self.config_addr())
            .field("alias_addr", &self.alias_addr())
            .field("eep_man", &self.eep_man())
            .field("eep_id", &self.eep_id())
            .field("eep_rev", &self.eep_rev())
            .field("output_bits", &self.output_bits())
            .field("output_bytes", &self.output_bytes())
            .field("outputs", &self.outputs())
            .field("input_bits", &self.input_bits())
            .field("input_bytes", &self.input_bytes())
            .field("inputs", &self.inputs())
            .field("has_dc", &self.has_dc())
            .field("parent_port", &self.parent_port())
            .field("propagation_delay", &self.propagation_delay())
            .field("group", &self.group())
            .field("name", &self.name())
            .finish()
    }
}
