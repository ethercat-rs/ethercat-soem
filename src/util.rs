use super::{Error, Result};
use ethercat_types::{DataType, Value};
use std::convert::TryInto;

// TODO: impl TryFrom<&[u8]> for Value in ethercat-types
pub fn value_from_slice(dt: DataType, raw: &[u8]) -> Result<Value> {
    debug_assert!(!raw.is_empty());
    if raw.is_empty() {
        return Err(Error::ValueFromEmptyBuf);
    }

    let val = match dt {
        DataType::Bool => Value::Bool(raw[0] == 1),
        DataType::Byte => Value::Byte(raw[0]),

        DataType::I8 => Value::I8(raw[0] as i8),
        DataType::I16 => Value::I16(i16::from_be_bytes(raw.try_into()?)),
        DataType::I32 => Value::I32(i32::from_be_bytes(raw.try_into()?)),
        DataType::I64 => Value::I64(i64::from_be_bytes(raw.try_into()?)),

        DataType::U8 => Value::U8(raw[0]),
        DataType::U16 => Value::U16(u16::from_be_bytes(raw.try_into()?)),
        DataType::U32 => Value::U32(u32::from_be_bytes(raw.try_into()?)),
        DataType::U64 => Value::U64(u64::from_be_bytes(raw.try_into()?)),

        DataType::F32 => Value::F32(f32::from_be_bytes(raw.try_into()?)),
        DataType::F64 => Value::F64(f64::from_be_bytes(raw.try_into()?)),

        DataType::String => Value::String(String::from_utf8_lossy(raw).to_string()),

        DataType::U8Array => Value::U8Array(raw.to_vec()),

        // TODO
        // U16Array

        // TODO:
        // I24
        // I40
        // I48
        // I56

        // TODO:
        // U24
        // U40
        // U48
        // U56

        // TODO:
        // Bit1
        // Bit2
        // Bit3
        // Bit4
        // Bit5
        // Bit6
        // Bit7
        // Bit8

        // TODO:
        // TimeOfDay
        // TimeDifference

        // TODO:
        // Domain
        DataType::Raw => Value::Raw(raw.to_vec()),

        _ => {
            return Err(Error::UnsuportedDataType(dt));
        }
    };
    Ok(val)
}
