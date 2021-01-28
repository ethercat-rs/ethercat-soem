use super::{Error, Result};
use ethercat_types::{DataType, Value};
use std::convert::TryInto;

// TODO: impl TryFrom<&[u8]> for Value in ethercat-types
pub fn value_from_slice(dt: DataType, raw: &[u8], bit_offset: usize) -> Result<Value> {
    debug_assert!(!raw.is_empty());
    if raw.is_empty() {
        return Err(Error::ValueFromEmptyBuf);
    }

    let val = match dt {
        DataType::Bool => {
            let bit_mask = 1 << bit_offset;
            Value::Bool((raw[0] & bit_mask) != 0)
        }
        DataType::Byte => Value::Byte(raw[0]),

        DataType::I8 => Value::I8(raw[0] as i8),
        DataType::I16 => Value::I16(i16::from_ne_bytes(raw.try_into()?)),
        DataType::I32 => Value::I32(i32::from_ne_bytes(raw.try_into()?)),
        DataType::I64 => Value::I64(i64::from_ne_bytes(raw.try_into()?)),

        DataType::U8 => Value::U8(raw[0]),
        DataType::U16 => Value::U16(u16::from_ne_bytes(raw.try_into()?)),
        DataType::U32 => Value::U32(u32::from_ne_bytes(raw.try_into()?)),
        DataType::U64 => Value::U64(u64::from_ne_bytes(raw.try_into()?)),

        DataType::F32 => Value::F32(f32::from_ne_bytes(raw.try_into()?)),
        DataType::F64 => Value::F64(f64::from_ne_bytes(raw.try_into()?)),

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

pub fn value_to_bytes(v: Value) -> Result<Vec<u8>> {
    use Value as V;

    let bytes = match v {
        V::Bool(b) => {
            if b {
                vec![1]
            } else {
                vec![0]
            }
        }
        V::Byte(v) => vec![v],

        V::I8(v) => v.to_ne_bytes().to_vec(),
        V::I16(v) => v.to_ne_bytes().to_vec(),
        V::I32(v) => v.to_ne_bytes().to_vec(),
        V::I64(v) => v.to_ne_bytes().to_vec(),

        V::U8(v) => v.to_ne_bytes().to_vec(),
        V::U16(v) => v.to_ne_bytes().to_vec(),
        V::U32(v) => v.to_ne_bytes().to_vec(),
        V::U64(v) => v.to_ne_bytes().to_vec(),

        V::F32(v) => v.to_ne_bytes().to_vec(),
        V::F64(v) => v.to_ne_bytes().to_vec(),

        V::Raw(raw) => raw,

        _ => {
            return Err(Error::UnsuportedValue(v));
        }
    };
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_from_raw_slice() {
        assert_eq!(
            value_from_slice(DataType::Bool, &[0], 0).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            value_from_slice(DataType::Bool, &[1], 0).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            value_from_slice(DataType::Bool, &[0b_1111_1111], 0).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            value_from_slice(DataType::Bool, &[0b_1111_1011], 2).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            value_from_slice(DataType::Bool, &[0b_1000_0000], 7).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            value_from_slice(DataType::Bool, &[0b_1000_0000], 0).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            value_from_slice(DataType::Bool, &[0b_0010_0000], 5).unwrap(),
            Value::Bool(true)
        );
    }
}
