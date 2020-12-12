use ethercat_soem_sys as sys;
use std::{fmt, mem::zeroed};

/// Max. entries in Object Entry list
const MAX_OE_LIST: usize = 256;

/// Storage for object list entry information
#[repr(C)]
pub struct OeList(pub(crate) sys::ec_OElistt);

impl Default for OeList {
    fn default() -> Self {
        Self(unsafe { zeroed() })
    }
}

impl OeList {
    /// Number of entries in list
    pub const fn entries(&self) -> usize {
        self.0.Entries as usize
    }
    /// Array of value infos, see EtherCAT specification
    pub fn value_info(&self) -> &[u8; MAX_OE_LIST] {
        &self.0.ValueInfo
    }
    /// Array of data types
    // TODO: map to ec::DataType
    pub fn data_types(&self) -> &[u16; MAX_OE_LIST] {
        &self.0.DataType
    }
    /// Array of bit lengths
    // TODO: map to usize
    pub fn bit_lengths(&self) -> &[u16; MAX_OE_LIST] {
        &self.0.BitLength
    }
    /// Array of object access bits
    // TODO: map ec::Access
    pub fn object_access(&self) -> &[u16; MAX_OE_LIST] {
        &self.0.ObjAccess
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

impl fmt::Debug for OeList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OeList")
            .field("entries", &self.entries())
            .field("value_info", &self.value_info())
            .field("data_types", &self.data_types())
            .field("bit_lengths", &self.bit_lengths())
            .field("object_access", &self.object_access())
            .field("names", &self.names())
            .finish()
    }
}
