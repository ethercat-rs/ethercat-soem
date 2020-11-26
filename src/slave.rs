use crate::{Error, Result};
use ethercat_soem_sys as sys;
use ethercat_types as ec;
use std::{borrow::Cow, convert::TryFrom, ffi::CStr, fmt, mem, slice, time::Duration};

#[repr(C)]
pub struct Slave(pub(crate) sys::ec_slave);

impl Default for Slave {
    fn default() -> Slave {
        Slave(unsafe { mem::zeroed() })
    }
}

impl Slave {
    pub fn name(&self) -> Cow<str> {
        let name_str = unsafe { CStr::from_ptr(self.0.name.as_ptr()) };
        name_str.to_string_lossy()
    }

    pub const fn output_bit_size(&self) -> usize {
        self.0.Obits as usize
    }

    pub const fn input_bit_size(&self) -> usize {
        self.0.Ibits as usize
    }

    pub fn outputs(&mut self) -> &mut [u8] {
        let size = (if self.0.Obytes == 0 && self.0.Obits > 0 {
            1
        } else {
            self.0.Obytes
        }) as usize;
        unsafe { slice::from_raw_parts_mut(self.0.outputs, size) }
    }

    pub fn inputs(&self) -> &[u8] {
        let size = (if self.0.Ibytes == 0 && self.0.Ibits > 0 {
            1
        } else {
            self.0.Ibytes
        }) as usize;
        unsafe { slice::from_raw_parts_mut(self.0.inputs, size) }
    }

    pub fn state(&self) -> Result<ec::AlState> {
        ec::AlState::try_from(self.0.state as u8).map_err(|_| Error::AlState)
    }

    pub fn set_state(&mut self, state: ec::AlState) {
        self.0.state = u8::from(state) as u16;
    }

    pub const fn delay(&self) -> Duration {
        Duration::from_nanos(self.0.pdelay as u64)
    }
    pub const fn has_dc(&self) -> bool {
        self.0.hasdc != 0
    }
    pub const fn manufacturer(&self) -> u32 {
        self.0.eep_man
    }
    pub const fn id(&self) -> u32 {
        self.0.eep_id
    }
    pub const fn revision(&self) -> u32 {
        self.0.eep_rev
    }
    pub const fn parent_port(&self) -> u8 {
        self.0.parentport
    }
    pub const fn configured_addr(&self) -> u16 {
        self.0.configadr
    }
}

impl fmt::Debug for Slave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Slave")
            .field("name", &self.name())
            .field("output_bit_size", &self.output_bit_size())
            .field("input_bit_size", &self.input_bit_size())
            .field("state", &self.state())
            .field("delay", &self.delay())
            .field("has_dc", &self.has_dc())
            .field("configured_addr", &self.configured_addr())
            .field("parent_port", &self.parent_port())
            .field("manufacturer", &self.manufacturer())
            .field("id", &self.id())
            .field("revision", &self.revision())
            .finish()
    }
}
