use num_derive::FromPrimitive;
use num_traits::cast::FromPrimitive as _;

// TODO: auto generate from `ethercatprint.c`

/// AL status code
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[repr(u16)]
pub enum AlStatus {
    /// No error
    NoError = 0x0000,

    /// Unspecified error
    UnspecifiedError = 0x0001,

    /// No memory,
    NoMemory = 0x0002,

    /// Invalid requested state change" },
    InvalidRequestedStateChange = 0x0011,

    /// Unknown requested state,
    UnknownRequestedState = 0x0012,

    /// Bootstrap not supported
    BootstrapNotSupported = 0x0013,

    /// No Valid Firmware
    NoValidFirmware = 0x0014,

    /// Invalid mailbox configuration
    ///
    /// First value as defined in `ethercatprint.c`.
    InvalidMailboxConfig = 0x0015,

    /// Invalid mailbox configuration
    ///
    /// Second value as defined in `ethercatprint.c`.
    InvalidMailboxConfig2 = 0x0016,

    /// Invalid sync manager configuration
    InvalidSyncManagerConfiguration = 0x0017,

    /// No valid inputs available
    NoValidInputsAvailable = 0x0018,

    /// No valid outputs
    NoValidOutputs = 0x0019,

    /// Synchronization error
    SynchronizationError = 0x001A,

    /// Sync manager watchdog
    SyncManagerWatchdog = 0x001B,

    /// Invalid sync Manager types,
    InvalidSyncManagerTypes = 0x001C,

    /// Invalid output configuration
    InvalidOutputConfiguration = 0x001D,

    /// Invalid input configuration
    InvalidInputConfiguration = 0x001E,

    /// Invalid watchdog configuration
    InvalidWatchdogConfiguration = 0x001F,

    /// Slave needs cold start
    SlaveNeedsColdStart = 0x0020,

    /// Slave needs INIT
    SlaveNeedsInit = 0x0021,

    /// Slave needs PREOP
    SlaveNeedsPreOp = 0x0022,

    /// Slave needs SAFEOP
    SlaveNeedsSafeOp = 0x0023,

    /// Invalid input mapping
    InvalidInputMapping = 0x0024,

    /// Invalid output mapping
    InvalidOutputMapping = 0x0025,

    /// Inconsistent settings
    InconsistentSettings = 0x0026,

    /// Freerun not supported
    FreerunNotSupported = 0x0027,

    /// Synchronisation not supported
    SynchronisationNotSupported = 0x0028,

    /// Freerun needs 3buffer mode
    FreerunNeeds3BufferMode = 0x0029,

    /// Background watchdog
    BackgroundWatchdog = 0x002A,

    /// No valid Inputs and Outputs
    NovalidInputsAndOutputs = 0x002B,

    /// Fatal sync error
    FatalSyncError = 0x002C,

    /// No sync error
    NoSyncError = 0x002D,

    /// Invalid input FMMU configuration
    InvalidInputFmmuConfiguration = 0x002E,

    /// Invalid DC SYNC configuration
    InvalidDcSyncConfiguration = 0x0030,

    /// Invalid DC latch configuration
    InvalidDcLatchConfiguration = 0x0031,

    /// PLL error
    PllError = 0x0032,

    /// DC sync IO error
    DcSyncIoError = 0x0033,

    /// DC sync timeout error
    DcSyncTimeoutError = 0x0034,

    /// DC invalid sync cycle time
    DcInvalidSyncCycleTime = 0x0035,

    /// DC invalid sync0 cycle time
    DcInvalidSync0CycleTime = 0x0036,

    /// DC invalid sync1 cycle time
    DcInvalidSync1CycleTime = 0x0037,

    /// MBX_AOE
    MbxAoe = 0x0041,

    /// MBX_EOE
    MbxEoe = 0x0042,

    /// MBX_COE
    MbxCoe = 0x0043,

    /// MBX_FOE
    MbxFoe = 0x0044,

    /// MBX_SOE
    MbxSoe = 0x0045,

    /// MBX_VOE
    MbcVoe = 0x004F,

    /// EEPROM no access
    EepromNoAccess = 0x0050,

    /// EEPROM error
    EeepromError = 0x0051,

    /// Slave restarted locally
    SlaveRestartedLocally = 0x0060,

    /// Device identification value updated
    DeviceIdValueUpdated = 0x0061,

    /// Application controller available
    ApplicationControllerAvailable = 0x00f0,

    /// Unknown
    Unknown = 0xffff,
}

impl From<u16> for AlStatus {
    fn from(code: u16) -> Self {
        AlStatus::from_u16(code).unwrap_or(AlStatus::Unknown)
    }
}
