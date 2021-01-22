use ethercat_soem_sys as sys;
use std::{fmt, mem};

/// SOEM `ec_sm` wrapper
#[repr(C)]
pub struct Sm(pub(crate) sys::ec_sm);

impl Default for Sm {
    fn default() -> Self {
        Self(unsafe { mem::zeroed() })
    }
}

impl Sm {
    pub const fn start_addr(&self) -> u16 {
        self.0.StartAddr
    }
    pub const fn len(&self) -> u16 {
        self.0.SMlength
    }
    pub const fn flags(&self) -> u32 {
        self.0.SMflags
    }
}

impl fmt::Debug for Sm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sm")
            .field("start_addr", &self.start_addr())
            .field("len", &self.len())
            .field("flags", &self.flags())
            .finish()
    }
}
