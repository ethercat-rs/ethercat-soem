use ethercat_soem_sys as sys;
use std::{fmt, mem::zeroed};

/// Max. entries in Object Description list
const MAX_OD_LIST: usize = 1024;

/// Storage for object description list
#[repr(C)]
pub struct OdList(pub(crate) sys::ec_ODlistt);

impl Default for OdList {
    fn default() -> Self {
        Self(unsafe { zeroed() })
    }
}

impl OdList {
    /// Slave position
    pub const fn slave(&self) -> u16 {
        self.0.Slave
    }
    /// Number of entries in list
    pub const fn entries(&self) -> usize {
        self.0.Entries as usize
    }
    /// Array of indexes
    pub const fn indexes(&self) -> &[u16; MAX_OD_LIST] {
        &self.0.Index
    }
    /// Array of data types
    pub const fn data_types(&self) -> &[u16; MAX_OD_LIST] {
        &self.0.DataType
    }
    /// Array of object codes
    pub const fn object_codes(&self) -> &[u8; MAX_OD_LIST] {
        &self.0.ObjectCode
    }
    /// Number of subindexes for each index
    pub const fn max_subs(&self) -> &[u8; MAX_OD_LIST] {
        &self.0.MaxSub
    }
    /// Textual description of each index
    pub fn names(&self) -> Vec<String> {
        self.0
            .Name
            .iter()
            .map(|slice| slice.as_ptr())
            .map(super::c_array_to_string)
            .collect()
    }
}

impl fmt::Debug for OdList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OdList")
            .field("slave", &self.slave())
            .field("entries", &self.entries())
            .field("indexes", &self.indexes())
            .field("data_types", &self.data_types())
            .field("object_codes", &self.object_codes())
            .field("max_subs", &self.max_subs())
            .field("names", &self.names())
            .finish()
    }
}
