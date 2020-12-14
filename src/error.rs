use crate::AlStatus;
use ethercat_types as ec;
use thiserror::Error;

/// Error
#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not init EtherCAT master")]
    Init,
    #[error("Invalid network interface")]
    Iface,
    #[error("No slaves found")]
    NoSlaves,
    #[error("Could not configure map group")]
    CfgMapGroup,
    #[error("Could not configure DC")]
    CfgDc,
    #[error("Could not set requested state")]
    SetState,
    #[error("Could not check state")]
    CheckState,
    #[error("Could not read states")]
    ReadStates,
    #[error("Could not send process data")]
    SendProcessData,
    #[error("Could not receive process data")]
    RecvProcessData,
    #[error("Invalid AL state: {0:?}")]
    AlState(AlStatus),
    #[error("Invalid group ID")]
    GroupId,
    #[error("Could not read OD list of {0:?}")]
    ReadOdList(ec::SlavePos),
    #[error("Could not read OD description of {0:?}")]
    ReadOdDesc(ec::SdoPos),
    #[error("Could not read OE list of {0:?}")]
    ReadOeList(ec::SdoPos),
    #[error("Could not read {1:?} of {0:?}")]
    ReadSdo(ec::SlavePos, ec::SdoIdx),
    #[error("Index {1:?} not found at {0:?}")]
    IdxNotFound(ec::SlavePos, ec::Idx),
    #[error("Subindex {1:?} not found at {0:?}")]
    SubIdxNotFound(ec::SlavePos, ec::SdoIdx),
    #[error("Could not write {1:?} of {0:?}")]
    WriteSdo(ec::SlavePos, ec::SdoIdx),
    #[error("Data type ({0:?}) is not supported yet")]
    UnsuportedDataType(ec::DataType),
    #[error("Cannot read value from empty buffer")]
    ValueFromEmptyBuf,
    #[error("Cannot convert raw data to EtherCAT value")]
    ValueConversion(#[from] std::array::TryFromSliceError),
    #[error("No frame received")]
    NoFrame,
    #[error("Unkown frame received")]
    OtherFrame,
}
