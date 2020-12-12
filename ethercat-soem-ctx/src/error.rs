use ethercat_soem_sys as sys;
use thiserror::Error;

/// SOEM `ec_errort` wrapper
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub err_type: ErrType,
    pub abort_code: i32,
    pub msg: String,
}

/// SOEM context error type
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum ErrType {
    #[error("Emergency")]
    Emergency,
    #[error("Packet")]
    Packet,
    #[error("Mailbox")]
    Mbx,
    #[error("SDO")]
    Sdo,
    #[error("SDO info")]
    SdoInfo,
    #[error("SOE")]
    Soe,
    #[error("FOE")]
    Foe,
    #[error("FOE packet number")]
    FoePacketNumber,
    #[error("FOE file not found")]
    FoeFileNotFound,
    #[error("FOE buf too small")]
    FoeBufTooSmall,
    #[error("EOE invalid RX data")]
    EoeInvalidRxData,
    #[error("Unknown error (type = {0})")]
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

impl From<sys::ec_errort> for Error {
    fn from(e: sys::ec_errort) -> Self {
        let err_type = ErrType::from(e.Etype);
        let abort_code = unsafe { e.__bindgen_anon_1.AbortCode };
        let msg = unsafe { super::c_array_to_string(sys::ecx_err2string(e)) };
        Error {
            err_type,
            abort_code,
            msg,
        }
    }
}
