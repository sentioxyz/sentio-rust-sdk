//! Direct serialization/deserialization between RichStruct and Entity
//!
//! This module provides direct conversion between protobuf RichStruct/RichValue types
//! and Rust types without using JSON as an intermediate format. It supports GraphQL
//! scalar types, Vec, and struct types for efficient data conversion.

use crate::common::{rich_value, RichStruct, RichValue, RichValueList};
use crate::core::conversions::{
    bigdecimal_to_proto, bigint_to_proto, proto_to_bigdecimal, proto_to_bigint,
};
use crate::entity::types::{BigDecimal, BigInt, Bytes, Timestamp, ID};
use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// Serialization error wrapper that implements serde::ser::Error and serde::de::Error
#[derive(Debug)]
pub struct SerdeError(pub String);

impl fmt::Display for SerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SerdeError {}

impl serde::ser::Error for SerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerdeError(format!("{}", msg))
    }
}

impl serde::de::Error for SerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerdeError(format!("{}", msg))
    }
}

/// Trait for direct serialization to RichValue without JSON intermediate
pub trait ToRichValue {
    fn to_rich_value(&self) -> Result<RichValue>;
}

/// Trait for direct deserialization from RichValue without JSON intermediate
pub trait FromRichValue: Sized {
    fn from_rich_value(value: &RichValue) -> Result<Self>;
}

/// Convert any serializable type to RichStruct using direct custom serializers
pub fn to_rich_struct<T: Serialize>(value: &T) -> Result<RichStruct> {
    let serializer = RichStructSerializer::new();
    match value.serialize(serializer) {
        Ok(rich_struct) => Ok(rich_struct),
        Err(e) => Err(anyhow::anyhow!("Direct serialization error: {}", e)),
    }
}

/// Convert RichStruct to any deserializable type using direct custom deserializers
pub fn from_rich_struct<T: DeserializeOwned>(rich_struct: &RichStruct) -> Result<T> {
    let deserializer = RichStructDeserializer::new(rich_struct);
    match T::deserialize(deserializer) {
        Ok(value) => Ok(value),
        Err(e) => Err(anyhow::anyhow!("Direct deserialization error: {}", e)),
    }
}

/// Convert any type implementing ToRichValue to RichValue directly
pub fn to_rich_value_direct<T: ToRichValue>(value: &T) -> Result<RichValue> {
    value.to_rich_value()
}

/// Convert RichValue to any type implementing FromRichValue directly
pub fn from_rich_value_direct<T: FromRichValue>(rich_value: &RichValue) -> Result<T> {
    T::from_rich_value(rich_value)
}

/// Convert struct with fields to RichStruct manually with pre-allocated capacity
pub fn struct_to_rich_struct(fields: Vec<(String, RichValue)>) -> RichStruct {
    let mut field_map = HashMap::with_capacity(fields.len()); // Pre-allocate for better performance
    for (key, value) in fields {
        field_map.insert(key, value);
    }
    RichStruct { fields: field_map }
}

/// Convert RichStruct to field vector
pub fn rich_struct_to_fields(rich_struct: &RichStruct) -> Vec<(String, &RichValue)> {
    rich_struct
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect()
}

// Protobuf conversions centralized in core::conversions

// GraphQL scalar type implementations

impl ToRichValue for String {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::StringValue(self.clone())),
        })
    }
}

impl FromRichValue for String {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::StringValue(s)) => Ok(s.clone()),
            _ => Err(anyhow::anyhow!("Expected string value")),
        }
    }
}

impl ToRichValue for &str {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::StringValue(self.to_string())),
        })
    }
}

impl ToRichValue for i32 {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::IntValue(*self)),
        })
    }
}

impl FromRichValue for i32 {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::IntValue(i)) => Ok(*i),
            _ => Err(anyhow::anyhow!("Expected int value")),
        }
    }
}

impl ToRichValue for i64 {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::Int64Value(*self)),
        })
    }
}

impl FromRichValue for i64 {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::Int64Value(i)) => Ok(*i),
            Some(rich_value::Value::IntValue(i)) => Ok(*i as i64),
            _ => Err(anyhow::anyhow!("Expected int64 value")),
        }
    }
}

impl ToRichValue for f64 {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::FloatValue(*self)),
        })
    }
}

impl FromRichValue for f64 {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::FloatValue(f)) => Ok(*f),
            Some(rich_value::Value::IntValue(i)) => Ok(*i as f64),
            Some(rich_value::Value::Int64Value(i)) => Ok(*i as f64),
            _ => Err(anyhow::anyhow!("Expected float value")),
        }
    }
}

impl ToRichValue for bool {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::BoolValue(*self)),
        })
    }
}

impl FromRichValue for bool {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::BoolValue(b)) => Ok(*b),
            _ => Err(anyhow::anyhow!("Expected bool value")),
        }
    }
}

impl ToRichValue for ID {
    fn to_rich_value(&self) -> Result<RichValue> {
        match self {
            ID::String(s) => s.to_rich_value(),
            ID::Int(i) => i.to_rich_value(),
            ID::Uuid(u) => u.to_string().to_rich_value(),
        }
    }
}

impl FromRichValue for ID {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::StringValue(s)) => {
                // Try to parse as UUID first, then as int, finally as string
                if let Ok(uuid) = uuid::Uuid::parse_str(s) {
                    Ok(ID::Uuid(uuid))
                } else if let Ok(int) = s.parse::<i64>() {
                    Ok(ID::Int(int))
                } else {
                    Ok(ID::String(s.clone()))
                }
            }
            Some(rich_value::Value::IntValue(i)) => Ok(ID::Int(*i as i64)),
            Some(rich_value::Value::Int64Value(i)) => Ok(ID::Int(*i)),
            _ => Err(anyhow::anyhow!("Expected ID value")),
        }
    }
}

impl ToRichValue for BigDecimal {
    fn to_rich_value(&self) -> Result<RichValue> {
        // Convert BigDecimal to protobuf BigDecimal format
        let proto_bigdecimal = bigdecimal_to_proto(self);
        Ok(RichValue {
            value: Some(rich_value::Value::BigdecimalValue(proto_bigdecimal)),
        })
    }
}

impl FromRichValue for BigDecimal {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::BigdecimalValue(proto_bigdecimal)) => {
                proto_to_bigdecimal(proto_bigdecimal)
            }
            Some(rich_value::Value::StringValue(s)) => s
                .parse::<BigDecimal>()
                .map_err(|e| anyhow::anyhow!("Invalid decimal: {}", e)),
            Some(rich_value::Value::FloatValue(f)) => {
                // Convert f64 to string first, then to BigDecimal for precision
                Ok(BigDecimal::from_str(&f.to_string())
                    .map_err(|e| anyhow::anyhow!("Invalid decimal from float: {}", e))?)
            }
            _ => Err(anyhow::anyhow!("Expected decimal value")),
        }
    }
}

impl ToRichValue for BigInt {
    fn to_rich_value(&self) -> Result<RichValue> {
        // Convert BigInt to protobuf BigInteger format
        let proto_biginteger = bigint_to_proto(self);
        Ok(RichValue {
            value: Some(rich_value::Value::BigintValue(proto_biginteger)),
        })
    }
}

impl FromRichValue for BigInt {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::BigintValue(proto_biginteger)) => {
                Ok(proto_to_bigint(proto_biginteger))
            }
            Some(rich_value::Value::StringValue(s)) => s
                .parse::<BigInt>()
                .map_err(|e| anyhow::anyhow!("Invalid big integer: {}", e)),
            Some(rich_value::Value::Int64Value(i)) => Ok(BigInt::from(*i)),
            Some(rich_value::Value::IntValue(i)) => Ok(BigInt::from(*i)),
            _ => Err(anyhow::anyhow!("Expected big integer value")),
        }
    }
}

impl ToRichValue for Timestamp {
    fn to_rich_value(&self) -> Result<RichValue> {
        let timestamp = prost_types::Timestamp {
            seconds: self.timestamp(),
            nanos: self.timestamp_subsec_nanos() as i32,
        };
        Ok(RichValue {
            value: Some(rich_value::Value::TimestampValue(timestamp)),
        })
    }
}

impl FromRichValue for Timestamp {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::TimestampValue(ts)) => {
                Timestamp::from_timestamp(ts.seconds, ts.nanos as u32)
                    .ok_or_else(|| anyhow::anyhow!("Invalid timestamp: seconds={}, nanos={}", ts.seconds, ts.nanos))
            }
            _ => Err(anyhow::anyhow!("Expected timestamp value")),
        }
    }
}

impl ToRichValue for Bytes {
    fn to_rich_value(&self) -> Result<RichValue> {
        Ok(RichValue {
            value: Some(rich_value::Value::BytesValue(self.to_vec())),
        })
    }
}

impl FromRichValue for Bytes {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::BytesValue(bytes)) => Ok(Bytes::from(bytes.clone())),
            _ => Err(anyhow::anyhow!("Expected bytes value")),
        }
    }
}

// Vec implementations
impl<T: ToRichValue> ToRichValue for Vec<T> {
    fn to_rich_value(&self) -> Result<RichValue> {
        let values: Result<Vec<_>> = self.iter().map(|item| item.to_rich_value()).collect();
        Ok(RichValue {
            value: Some(rich_value::Value::ListValue(RichValueList {
                values: values?,
            })),
        })
    }
}

impl<T: FromRichValue> FromRichValue for Vec<T> {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::ListValue(list)) => {
                let items: Result<Vec<_>> =
                    list.values.iter().map(|v| T::from_rich_value(v)).collect();
                items
            }
            _ => Err(anyhow::anyhow!("Expected list value")),
        }
    }
}

// Option implementations
impl<T: ToRichValue> ToRichValue for Option<T> {
    fn to_rich_value(&self) -> Result<RichValue> {
        match self {
            Some(value) => value.to_rich_value(),
            None => Ok(RichValue {
                value: Some(rich_value::Value::NullValue(
                    rich_value::NullValue::NullValue as i32,
                )),
            }),
        }
    }
}

impl<T: FromRichValue> FromRichValue for Option<T> {
    fn from_rich_value(value: &RichValue) -> Result<Self> {
        match &value.value {
            Some(rich_value::Value::NullValue(_)) | None => Ok(None),
            _ => Ok(Some(T::from_rich_value(value)?)),
        }
    }
}

// COMPLETE CUSTOM SERDE SERIALIZERS - NO JSON BRIDGE

/// Main serializer that converts any serde::Serialize to RichStruct
pub struct RichStructSerializer {
    _phantom: std::marker::PhantomData<()>,
}

impl Default for RichStructSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichStructSerializer {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl serde::Serializer for RichStructSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    type SerializeSeq = RichSeqSerializer;
    type SerializeTuple = RichTupleSerializer;
    type SerializeTupleStruct = RichTupleStructSerializer;
    type SerializeTupleVariant = RichTupleVariantSerializer;
    type SerializeMap = RichMapSerializer;
    type SerializeStruct = RichStructSerializerImpl;
    type SerializeStructVariant = RichStructVariantSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        // Convert single bool to struct with "value" field
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::BoolValue(v)),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::IntValue(v)),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::Int64Value(v)),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::FloatValue(v)),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(v.to_string())),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::BytesValue(v.to_vec())),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "value".to_string(),
            RichValue {
                value: Some(rich_value::Value::NullValue(
                    rich_value::NullValue::NullValue as i32,
                )),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichStruct {
            fields: HashMap::new(),
        })
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(RichStruct {
            fields: HashMap::new(),
        })
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "variant".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(variant.to_string())),
            },
        );
        Ok(RichStruct { fields })
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        let mut fields = HashMap::new();
        fields.insert(
            "variant".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(variant.to_string())),
            },
        );

        let value_serializer = RichValueSerializer::new();
        let rich_value = value.serialize(value_serializer)?;
        fields.insert("value".to_string(), rich_value);

        Ok(RichStruct { fields })
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(RichSeqSerializer::new())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(RichTupleSerializer::new())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(RichTupleStructSerializer::new())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(RichTupleVariantSerializer::new(variant))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(RichMapSerializer::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(RichStructSerializerImpl::new())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(RichStructVariantSerializer::new(variant))
    }
}

/// Serializer for RichValue (used for nested values)
pub struct RichValueSerializer {
    _phantom: std::marker::PhantomData<()>,
}

impl Default for RichValueSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueSerializer {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl serde::Serializer for RichValueSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    type SerializeSeq = RichValueSeqSerializer;
    type SerializeTuple = RichValueTupleSerializer;
    type SerializeTupleStruct = RichValueTupleStructSerializer;
    type SerializeTupleVariant = RichValueTupleVariantSerializer;
    type SerializeMap = RichValueMapSerializer;
    type SerializeStruct = RichValueStructSerializer;
    type SerializeStructVariant = RichValueStructVariantSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::BoolValue(v)),
        })
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::IntValue(v)),
        })
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::Int64Value(v)),
        })
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::FloatValue(v)),
        })
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::StringValue(v.to_string())),
        })
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::BytesValue(v.to_vec())),
        })
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::NullValue(
                rich_value::NullValue::NullValue as i32,
            )),
        })
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(RichValueSeqSerializer::new())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(RichValueTupleSerializer::new())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(RichValueTupleStructSerializer::new())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(RichValueTupleVariantSerializer::new())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(RichValueMapSerializer::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(RichValueStructSerializer::new())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(RichValueStructVariantSerializer::new(_variant))
    }
}

// SEQUENCE SERIALIZERS

pub struct RichSeqSerializer {
    values: Vec<RichValue>,
}

impl Default for RichSeqSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichSeqSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self { values: Vec::with_capacity(capacity) }
    }
}

impl serde::ser::SerializeSeq for RichSeqSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "values".to_string(),
            RichValue {
                value: Some(rich_value::Value::ListValue(RichValueList {
                    values: self.values,
                })),
            },
        );
        Ok(RichStruct { fields })
    }
}

pub struct RichValueSeqSerializer {
    values: Vec<RichValue>,
}

impl Default for RichValueSeqSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueSeqSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self { values: Vec::with_capacity(capacity) }
    }
}

impl serde::ser::SerializeSeq for RichValueSeqSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::ListValue(RichValueList {
                values: self.values,
            })),
        })
    }
}

// TUPLE SERIALIZERS (similar pattern for each)

pub struct RichTupleSerializer {
    values: Vec<RichValue>,
}

impl Default for RichTupleSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichTupleSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self { values: Vec::with_capacity(capacity) }
    }
}

impl serde::ser::SerializeTuple for RichTupleSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "tuple".to_string(),
            RichValue {
                value: Some(rich_value::Value::ListValue(RichValueList {
                    values: self.values,
                })),
            },
        );
        Ok(RichStruct { fields })
    }
}

pub struct RichValueTupleSerializer {
    values: Vec<RichValue>,
}

impl Default for RichValueTupleSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueTupleSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self { values: Vec::with_capacity(capacity) }
    }
}

impl serde::ser::SerializeTuple for RichValueTupleSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::ListValue(RichValueList {
                values: self.values,
            })),
        })
    }
}

// TUPLE STRUCT SERIALIZERS

pub struct RichTupleStructSerializer {
    values: Vec<RichValue>,
}

impl Default for RichTupleStructSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichTupleStructSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl serde::ser::SerializeTupleStruct for RichTupleStructSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "fields".to_string(),
            RichValue {
                value: Some(rich_value::Value::ListValue(RichValueList {
                    values: self.values,
                })),
            },
        );
        Ok(RichStruct { fields })
    }
}

pub struct RichValueTupleStructSerializer {
    values: Vec<RichValue>,
}

impl Default for RichValueTupleStructSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueTupleStructSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl serde::ser::SerializeTupleStruct for RichValueTupleStructSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::ListValue(RichValueList {
                values: self.values,
            })),
        })
    }
}

// TUPLE VARIANT SERIALIZERS

pub struct RichTupleVariantSerializer {
    variant: String,
    values: Vec<RichValue>,
}

impl RichTupleVariantSerializer {
    pub fn new(variant: &str) -> Self {
        Self {
            variant: variant.to_string(),
            values: Vec::new(),
        }
    }
}

impl serde::ser::SerializeTupleVariant for RichTupleVariantSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut fields = HashMap::new();
        fields.insert(
            "variant".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(self.variant)),
            },
        );
        fields.insert(
            "fields".to_string(),
            RichValue {
                value: Some(rich_value::Value::ListValue(RichValueList {
                    values: self.values,
                })),
            },
        );
        Ok(RichStruct { fields })
    }
}

pub struct RichValueTupleVariantSerializer {
    values: Vec<RichValue>,
}

impl Default for RichValueTupleVariantSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueTupleVariantSerializer {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl serde::ser::SerializeTupleVariant for RichValueTupleVariantSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.values.push(rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::ListValue(RichValueList {
                values: self.values,
            })),
        })
    }
}

// MAP SERIALIZERS

pub struct RichMapSerializer {
    fields: HashMap<String, RichValue>,
    current_key: Option<String>,
}

impl Default for RichMapSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichMapSerializer {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            current_key: None,
        }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            fields: HashMap::with_capacity(capacity),
            current_key: None,
        }
    }
}

impl serde::ser::SerializeMap for RichMapSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key_serializer = RichValueSerializer::new();
        let key_value = key.serialize(key_serializer)?;

        // Extract string from RichValue
        match key_value.value {
            Some(rich_value::Value::StringValue(s)) => {
                self.current_key = Some(s);
                Ok(())
            }
            _ => Err(SerdeError("Map keys must be strings".to_string())),
        }
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key = self
            .current_key
            .take()
            .ok_or_else(|| SerdeError("Missing key for map value".to_string()))?;
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.fields.insert(key, rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichStruct {
            fields: self.fields,
        })
    }
}

pub struct RichValueMapSerializer {
    fields: HashMap<String, RichValue>,
    current_key: Option<String>,
}

impl Default for RichValueMapSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueMapSerializer {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            current_key: None,
        }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            fields: HashMap::with_capacity(capacity),
            current_key: None,
        }
    }
}

impl serde::ser::SerializeMap for RichValueMapSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key_serializer = RichValueSerializer::new();
        let key_value = key.serialize(key_serializer)?;

        // Extract string from RichValue
        match key_value.value {
            Some(rich_value::Value::StringValue(s)) => {
                self.current_key = Some(s);
                Ok(())
            }
            _ => Err(SerdeError("Map keys must be strings".to_string())),
        }
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key = self
            .current_key
            .take()
            .ok_or_else(|| SerdeError("Missing key for map value".to_string()))?;
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.fields.insert(key, rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichValue {
            value: Some(rich_value::Value::StructValue(RichStruct {
                fields: self.fields,
            })),
        })
    }
}

// STRUCT SERIALIZERS - THE MOST IMPORTANT ONES

pub struct RichStructSerializerImpl {
    fields: HashMap<String, RichValue>,
}

impl Default for RichStructSerializerImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl RichStructSerializerImpl {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            fields: HashMap::with_capacity(capacity),
        }
    }
}

impl serde::ser::SerializeStruct for RichStructSerializerImpl {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.fields.insert(key.to_string(), rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(RichStruct {
            fields: self.fields,
        })
    }
}

pub struct RichValueStructSerializer {
    fields: HashMap<String, RichValue>,
}

impl Default for RichValueStructSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichValueStructSerializer {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }
    
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            fields: HashMap::with_capacity(capacity),
        }
    }
}

impl serde::ser::SerializeStruct for RichValueStructSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.fields.insert(key.to_string(), rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // Check if this is a Timestamp struct (has "seconds" and "nanos" fields)
        if self.fields.len() == 2 
            && self.fields.contains_key("seconds") 
            && self.fields.contains_key("nanos") {
            
            // Extract seconds and nanos values
            if let (Some(seconds_value), Some(nanos_value)) = 
                (self.fields.get("seconds"), self.fields.get("nanos"))
                
                && let (Some(rich_value::Value::Int64Value(seconds)), Some(rich_value::Value::IntValue(nanos))) = 
                    (&seconds_value.value, &nanos_value.value) {
                    
                    // Create protobuf Timestamp
                    let prost_timestamp = prost_types::Timestamp {
                        seconds: *seconds,
                        nanos: *nanos,
                    };
                    
                    return Ok(RichValue {
                        value: Some(rich_value::Value::TimestampValue(prost_timestamp)),
                    });
                }
        }
        
        // Default struct serialization
        Ok(RichValue {
            value: Some(rich_value::Value::StructValue(RichStruct {
                fields: self.fields,
            })),
        })
    }
}

// STRUCT VARIANT SERIALIZERS

pub struct RichStructVariantSerializer {
    variant: String,
    fields: HashMap<String, RichValue>,
}

impl RichStructVariantSerializer {
    pub fn new(variant: &str) -> Self {
        Self {
            variant: variant.to_string(),
            fields: HashMap::new(),
        }
    }
}

impl serde::ser::SerializeStructVariant for RichStructVariantSerializer {
    type Ok = RichStruct;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.fields.insert(key.to_string(), rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut result_fields = HashMap::new();
        result_fields.insert(
            "variant".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(self.variant)),
            },
        );
        result_fields.insert(
            "fields".to_string(),
            RichValue {
                value: Some(rich_value::Value::StructValue(RichStruct {
                    fields: self.fields,
                })),
            },
        );
        Ok(RichStruct {
            fields: result_fields,
        })
    }
}

pub struct RichValueStructVariantSerializer {
    variant: String,
    fields: HashMap<String, RichValue>,
}

impl RichValueStructVariantSerializer {
    pub fn new(variant: &str) -> Self {
        Self {
            variant: variant.to_string(),
            fields: HashMap::new(),
        }
    }
}

impl serde::ser::SerializeStructVariant for RichValueStructVariantSerializer {
    type Ok = RichValue;
    type Error = SerdeError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = RichValueSerializer::new();
        let rich_value = value.serialize(serializer)?;
        self.fields.insert(key.to_string(), rich_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut result_fields = HashMap::new();
        result_fields.insert(
            "variant".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(self.variant)),
            },
        );
        result_fields.insert(
            "fields".to_string(),
            RichValue {
                value: Some(rich_value::Value::StructValue(RichStruct {
                    fields: self.fields,
                })),
            },
        );
        Ok(RichValue {
            value: Some(rich_value::Value::StructValue(RichStruct {
                fields: result_fields,
            })),
        })
    }
}

// DESERIALIZER IMPLEMENTATION

pub struct RichStructDeserializer<'a> {
    rich_struct: &'a RichStruct,
}

impl<'a> RichStructDeserializer<'a> {
    pub fn new(rich_struct: &'a RichStruct) -> Self {
        Self { rich_struct }
    }
}

impl<'de, 'a> serde::Deserializer<'de> for RichStructDeserializer<'a> {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // Check for single-value struct with "value" field
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::BoolValue(b)) => visitor.visit_bool(*b),
                _ => Err(SerdeError("Expected bool value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for bool deserialization".to_string(),
            ))
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_i32(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_i32(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::IntValue(i)) => visitor.visit_i32(*i),
                _ => Err(SerdeError("Expected int value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for i32 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::Int64Value(i)) => visitor.visit_i64(*i),
                Some(rich_value::Value::IntValue(i)) => visitor.visit_i64(*i as i64),
                _ => Err(SerdeError("Expected int64 value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for i64 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::IntValue(i)) => visitor.visit_u8(*i as u8),
                _ => Err(SerdeError("Expected int value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for u8 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::IntValue(i)) => visitor.visit_u16(*i as u16),
                _ => Err(SerdeError("Expected int value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for u16 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::Int64Value(i)) => visitor.visit_u32(*i as u32),
                Some(rich_value::Value::IntValue(i)) => visitor.visit_u32(*i as u32),
                _ => Err(SerdeError("Expected int64 value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for u32 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::Int64Value(i)) => visitor.visit_u64(*i as u64),
                Some(rich_value::Value::IntValue(i)) => visitor.visit_u64(*i as u64),
                _ => Err(SerdeError("Expected int64 value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for u64 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::FloatValue(f)) => visitor.visit_f64(*f),
                Some(rich_value::Value::IntValue(i)) => visitor.visit_f64(*i as f64),
                Some(rich_value::Value::Int64Value(i)) => visitor.visit_f64(*i as f64),
                _ => Err(SerdeError("Expected float value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for f64 deserialization".to_string(),
            ))
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::StringValue(s)) => visitor.visit_str(s),
                _ => Err(SerdeError("Expected string value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for str deserialization".to_string(),
            ))
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::StringValue(s)) => visitor.visit_string(s.clone()),
                _ => Err(SerdeError("Expected string value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for string deserialization".to_string(),
            ))
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::BytesValue(bytes)) => visitor.visit_bytes(bytes),
                _ => Err(SerdeError("Expected bytes value".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'value' field for bytes deserialization".to_string(),
            ))
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("value") {
            match &value.value {
                Some(rich_value::Value::NullValue(_)) => visitor.visit_none(),
                _ => visitor.visit_some(self),
            }
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("values") {
            match &value.value {
                Some(rich_value::Value::ListValue(list)) => {
                    let seq_access = RichValueListSeqAccess::new(&list.values);
                    visitor.visit_seq(seq_access)
                }
                _ => Err(SerdeError("Expected list value for sequence".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'values' field for sequence deserialization".to_string(),
            ))
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("tuple") {
            match &value.value {
                Some(rich_value::Value::ListValue(list)) => {
                    let seq_access = RichValueListSeqAccess::new(&list.values);
                    visitor.visit_seq(seq_access)
                }
                _ => Err(SerdeError("Expected list value for tuple".to_string())),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'tuple' field for tuple deserialization".to_string(),
            ))
        }
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let Some(value) = self.rich_struct.fields.get("fields") {
            match &value.value {
                Some(rich_value::Value::ListValue(list)) => {
                    let seq_access = RichValueListSeqAccess::new(&list.values);
                    visitor.visit_seq(seq_access)
                }
                _ => Err(SerdeError(
                    "Expected list value for tuple struct".to_string(),
                )),
            }
        } else {
            Err(SerdeError(
                "Expected struct with 'fields' field for tuple struct deserialization".to_string(),
            ))
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let map_access = RichStructMapAccess::new(self.rich_struct);
        visitor.visit_map(map_access)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let map_access = RichStructMapAccess::new(self.rich_struct);
        visitor.visit_map(map_access)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(SerdeError(
            "Enum deserialization not implemented yet".to_string(),
        ))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

// MAP ACCESS FOR DESERIALIZATION

pub struct RichStructMapAccess<'a> {
    rich_struct: &'a RichStruct,
    keys: std::vec::IntoIter<String>,
    current_key: Option<String>,
}

impl<'a> RichStructMapAccess<'a> {
    pub fn new(rich_struct: &'a RichStruct) -> Self {
        let keys: Vec<String> = rich_struct.fields.keys().cloned().collect();
        Self {
            rich_struct,
            keys: keys.into_iter(),
            current_key: None,
        }
    }
}

impl<'de, 'a> serde::de::MapAccess<'de> for RichStructMapAccess<'a> {
    type Error = SerdeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.keys.next() {
            Some(key) => {
                self.current_key = Some(key.clone());
                let key_deserializer = StringDeserializer::new(&key);
                seed.deserialize(key_deserializer).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let key = self
            .current_key
            .take()
            .ok_or_else(|| SerdeError("Missing key for map value".to_string()))?;
        let value = self
            .rich_struct
            .fields
            .get(&key)
            .ok_or_else(|| SerdeError(format!("Missing field: {}", key)))?;
        let value_deserializer = RichValueDeserializer::new(value);
        seed.deserialize(value_deserializer)
    }
}

// SEQUENCE ACCESS FOR DESERIALIZATION

pub struct RichValueListSeqAccess<'a> {
    values: &'a [RichValue],
    index: usize,
}

impl<'a> RichValueListSeqAccess<'a> {
    pub fn new(values: &'a [RichValue]) -> Self {
        Self { values, index: 0 }
    }
}

impl<'de, 'a> serde::de::SeqAccess<'de> for RichValueListSeqAccess<'a> {
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if self.index >= self.values.len() {
            return Ok(None);
        }

        let value = &self.values[self.index];
        self.index += 1;

        let deserializer = RichValueDeserializer::new(value);
        seed.deserialize(deserializer).map(Some)
    }
}

// RICHVALUE DESERIALIZER

pub struct RichValueDeserializer<'a> {
    rich_value: &'a RichValue,
}

impl<'a> RichValueDeserializer<'a> {
    pub fn new(rich_value: &'a RichValue) -> Self {
        Self { rich_value }
    }
}

impl<'de, 'a> serde::Deserializer<'de> for RichValueDeserializer<'a> {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match &self.rich_value.value {
            Some(rich_value::Value::StringValue(s)) => visitor.visit_string(s.clone()),
            Some(rich_value::Value::IntValue(i)) => visitor.visit_i32(*i),
            Some(rich_value::Value::Int64Value(i)) => visitor.visit_i64(*i),
            Some(rich_value::Value::FloatValue(f)) => visitor.visit_f64(*f),
            Some(rich_value::Value::BoolValue(b)) => visitor.visit_bool(*b),
            Some(rich_value::Value::NullValue(_)) => visitor.visit_none(),
            Some(rich_value::Value::ListValue(list)) => {
                let seq_access = RichValueListSeqAccess::new(&list.values);
                visitor.visit_seq(seq_access)
            }
            Some(rich_value::Value::StructValue(struct_val)) => {
                let map_access = RichStructMapAccess::new(struct_val);
                visitor.visit_map(map_access)
            }
            Some(rich_value::Value::BytesValue(bytes)) => visitor.visit_bytes(bytes),
            Some(rich_value::Value::TimestampValue(_)) => {
                // Handle timestamp deserialization
                match Timestamp::from_rich_value(self.rich_value) {
                    Ok(timestamp) => visitor.visit_string(timestamp.to_rfc3339()),
                    Err(e) => Err(SerdeError(format!(
                        "Timestamp deserialization error: {}",
                        e
                    ))),
                }
            }
            Some(rich_value::Value::BigintValue(_)) => {
                // Handle BigInt deserialization - use string format for simplicity
                match BigInt::from_rich_value(self.rich_value) {
                    Ok(big_int) => {
                        // visitor.visit_string(big_int.to_string())
                        // Convert BigInt to tuple format (sign, data) that serde expects
                        use num_bigint::Sign;
                        let (sign, biguint) = big_int.into_parts();

                        // Create a sequence with (sign, biguint) tuple
                        let tuple_values = vec![
                            match sign {
                                Sign::Plus => RichValue {
                                    value: Some(rich_value::Value::IntValue(1)),
                                },
                                Sign::Minus => RichValue {
                                    value: Some(rich_value::Value::IntValue(-1)),
                                },
                                Sign::NoSign => RichValue {
                                    value: Some(rich_value::Value::IntValue(0)),
                                },
                            },
                            // For BigUint, we need to convert it to u32 sequence for serde
                            {
                                let u32_digits: Vec<u32> = biguint.to_u32_digits();
                                let u32_values: Vec<RichValue> = u32_digits
                                    .iter()
                                    .map(|&digit| RichValue {
                                        value: Some(rich_value::Value::Int64Value(digit as i64)),
                                    })
                                    .collect();

                                RichValue {
                                    value: Some(rich_value::Value::ListValue(RichValueList {
                                        values: u32_values,
                                    })),
                                }
                            },
                        ];

                        let seq_access = RichValueListSeqAccess::new(&tuple_values);
                        visitor.visit_seq(seq_access)
                    }
                    Err(e) => Err(SerdeError(format!("BigInt deserialization error: {}", e))),
                }
            }
            Some(rich_value::Value::BigdecimalValue(_)) => {
                // Handle BigDecimal deserialization
                match BigDecimal::from_rich_value(self.rich_value) {
                    Ok(big_decimal) => visitor.visit_string(big_decimal.to_string()),
                    Err(e) => Err(SerdeError(format!(
                        "BigDecimal deserialization error: {}",
                        e
                    ))),
                }
            }
            None => visitor.visit_none(),
            _ => Err(SerdeError("Unsupported RichValue type".to_string())),
        }
    }

    // Custom implementation for option to handle values correctly
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match &self.rich_value.value {
            Some(rich_value::Value::NullValue(_)) | None => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    // Implement specific type deserializers
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

// STRING DESERIALIZER

pub struct StringDeserializer {
    s: String,
}

impl StringDeserializer {
    pub fn new(s: &str) -> Self {
        Self { s: s.to_string() }
    }
}

impl<'de> serde::Deserializer<'de> for StringDeserializer {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.s)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.s)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_str(&self.s)
    }

    // Implement other methods by delegating to deserialize_any
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char bytes
        byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

// BIGINT AND BIGDECIMAL DESERIALIZERS

pub struct BigIntDeserializer {
    value: BigInt,
}

impl BigIntDeserializer {
    pub fn new(value: BigInt) -> Self {
        Self { value }
    }
}

impl<'de> serde::Deserializer<'de> for BigIntDeserializer {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // For BigInt, we need to return it as a newtype struct
        // Convert to string for serde compatibility
        visitor.visit_string(self.value.to_string())
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // For BigInt newtype struct deserialization
        visitor.visit_string(self.value.to_string())
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

pub struct BigDecimalDeserializer {
    value: BigDecimal,
}

impl BigDecimalDeserializer {
    pub fn new(value: BigDecimal) -> Self {
        Self { value }
    }
}

impl<'de> serde::Deserializer<'de> for BigDecimalDeserializer {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // For BigDecimal, we need to return it as a newtype struct
        // Convert to string for serde compatibility
        visitor.visit_string(self.value.to_string())
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // For BigDecimal newtype struct deserialization
        visitor.visit_string(self.value.to_string())
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use serde::{Deserialize, Serialize};
    use std::str::FromStr;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestEntity {
        pub id: String,
        pub name: String,
        pub value: i32,
        pub score: f64,
        pub active: bool,
        pub tags: Vec<String>,
        pub optional_field: Option<i32>,
    }

    #[test]
    fn test_direct_scalar_conversion() {
        // Test string conversion
        let s = "hello".to_string();
        let rich_value = s.to_rich_value().unwrap();
        let converted_back = String::from_rich_value(&rich_value).unwrap();
        assert_eq!(s, converted_back);

        // Test i32 conversion
        let i = 42i32;
        let rich_value = i.to_rich_value().unwrap();
        let converted_back = i32::from_rich_value(&rich_value).unwrap();
        assert_eq!(i, converted_back);

        // Test f64 conversion
        let f = 3.14f64;
        let rich_value = f.to_rich_value().unwrap();
        let converted_back = f64::from_rich_value(&rich_value).unwrap();
        assert_eq!(f, converted_back);

        // Test bool conversion
        let b = true;
        let rich_value = b.to_rich_value().unwrap();
        let converted_back = bool::from_rich_value(&rich_value).unwrap();
        assert_eq!(b, converted_back);
    }

    #[test]
    fn test_vec_conversion() {
        let vec = vec!["hello".to_string(), "world".to_string()];
        let rich_value = vec.to_rich_value().unwrap();
        let converted_back = Vec::<String>::from_rich_value(&rich_value).unwrap();
        assert_eq!(vec, converted_back);
    }

    #[test]
    fn test_option_conversion() {
        // Test Some value
        let opt_some = Some(42i32);
        let rich_value = opt_some.to_rich_value().unwrap();
        let converted_back = Option::<i32>::from_rich_value(&rich_value).unwrap();
        assert_eq!(opt_some, converted_back);

        // Test None value
        let opt_none: Option<i32> = None;
        let rich_value = opt_none.to_rich_value().unwrap();
        let converted_back = Option::<i32>::from_rich_value(&rich_value).unwrap();
        assert_eq!(opt_none, converted_back);
    }

    #[test]
    fn test_timestamp_conversion() {
        // Create a test timestamp
        let original_timestamp = Timestamp::from_timestamp(1672531200, 123456789)
            .expect("Valid timestamp");

        // Convert to RichValue (this is the serialization step)
        let rich_value = original_timestamp.to_rich_value().unwrap();
        
        // Verify the structure contains the correct protobuf timestamp
        match &rich_value.value {
            Some(rich_value::Value::TimestampValue(ts)) => {
                assert_eq!(ts.seconds, 1672531200);
                assert_eq!(ts.nanos, 123456789);
            }
            _ => panic!("Expected TimestampValue variant"),
        }

        // Convert back from RichValue (this is the deserialization step)
        let converted_back = Timestamp::from_rich_value(&rich_value).unwrap();
        
        // Verify round-trip conversion
        assert_eq!(original_timestamp.timestamp(), converted_back.timestamp());
        assert_eq!(original_timestamp.timestamp_subsec_nanos(), converted_back.timestamp_subsec_nanos());
        assert_eq!(original_timestamp, converted_back);
    }

    #[test]
    fn test_entity_serialization_direct_no_json() {
        let entity = TestEntity {
            id: "test-123".to_string(),
            name: "Test Entity".to_string(),
            value: 42,
            score: 3.14,
            active: true,
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            optional_field: Some(100),
        };

        // Test DIRECT serialization to RichStruct (NO JSON!)
        let rich_struct = to_rich_struct(&entity).unwrap();

        // Verify the structure contains fields directly
        assert!(rich_struct.fields.contains_key("id"));
        assert!(rich_struct.fields.contains_key("name"));
        assert!(rich_struct.fields.contains_key("value"));
        assert!(rich_struct.fields.contains_key("score"));
        assert!(rich_struct.fields.contains_key("active"));
        assert!(rich_struct.fields.contains_key("tags"));
        assert!(rich_struct.fields.contains_key("optional_field"));

        // Test DIRECT deserialization back to entity (NO JSON!)
        let converted_entity: TestEntity = from_rich_struct(&rich_struct).unwrap();
        assert_eq!(entity, converted_entity);
    }

    #[test]
    fn test_manual_struct_creation() {
        let fields = vec![
            (
                "name".to_string(),
                "Test".to_string().to_rich_value().unwrap(),
            ),
            ("value".to_string(), 42i32.to_rich_value().unwrap()),
            ("active".to_string(), true.to_rich_value().unwrap()),
        ];

        let rich_struct = struct_to_rich_struct(fields);
        let field_list = rich_struct_to_fields(&rich_struct);

        assert_eq!(field_list.len(), 3);
        assert!(field_list.iter().any(|(k, _)| k == "name"));
        assert!(field_list.iter().any(|(k, _)| k == "value"));
        assert!(field_list.iter().any(|(k, _)| k == "active"));
    }

    #[test]
    fn test_id_conversion() {
        // Test string ID
        let string_id = ID::String("test-123".to_string());
        let rich_value = string_id.to_rich_value().unwrap();
        let converted_back = ID::from_rich_value(&rich_value).unwrap();
        assert_eq!(string_id, converted_back);

        // Test int ID
        let int_id = ID::Int(42);
        let rich_value = int_id.to_rich_value().unwrap();
        let converted_back = ID::from_rich_value(&rich_value).unwrap();
        assert_eq!(int_id, converted_back);

        // Test UUID ID
        let uuid = uuid::Uuid::new_v4();
        let uuid_id = ID::Uuid(uuid);
        let rich_value = uuid_id.to_rich_value().unwrap();
        let converted_back = ID::from_rich_value(&rich_value).unwrap();
        assert_eq!(uuid_id, converted_back);
    }

    #[test]
    fn test_primitive_to_struct_conversion() {
        // Test that primitive values are wrapped in structs with "value" field
        let number = 42i32;
        let rich_struct = to_rich_struct(&number).unwrap();

        assert!(rich_struct.fields.contains_key("value"));
        if let Some(rich_value) = rich_struct.fields.get("value") {
            match &rich_value.value {
                Some(rich_value::Value::IntValue(i)) => assert_eq!(*i, 42),
                _ => panic!("Expected IntValue"),
            }
        }

        // Test round-trip
        let converted_back: i32 = from_rich_struct(&rich_struct).unwrap();
        assert_eq!(number, converted_back);
    }

    #[test]
    fn test_big_integer_conversion() {
        // Test BigInt from string
        let big_int = BigInt::from(123456789012345u64);
        let rich_value = big_int.to_rich_value().unwrap();
        let converted_back = BigInt::from_rich_value(&rich_value).unwrap();
        assert_eq!(big_int, converted_back);

        // Test BigInt from very large number (string representation)
        let large_str = "123456789012345678901234567890";
        let large_big_int = large_str.parse::<BigInt>().unwrap();
        let rich_value = large_big_int.to_rich_value().unwrap();
        let converted_back = BigInt::from_rich_value(&rich_value).unwrap();
        assert_eq!(large_big_int, converted_back);
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestEntityWithBigTypesAndBytes {
        pub id: String,
        pub name: String,
        #[serde(rename = "bigInt")]
        pub big_int: BigInt,
        #[serde(rename = "negBigInt")]
        pub neg_big_int: BigInt,
        #[serde(rename = "bigDecimal")]
        pub big_decimal: BigDecimal,
        #[serde(rename = "negBigDecimal")]
        pub neg_big_decimal: BigDecimal,
        #[serde(rename = "isActive")]
        pub is_active: bool,
        pub tags: Vec<String>,
        pub data: Bytes,
    }
    use crate::common::RichStruct;
    use ethers::utils::hex;

    #[test]
    fn test_deserialize_from_bin_data() {
        let hex_bytes = "0a0c0a02696412063204746573740a150a046e616d65120d320b5465737420456e746974790a270a06626967496e74121d421b121913aaf504e4bc1e62173f87a4378c37b49c8ccff196ce3f0ad20a2c0a096e6567426967496e74121f421d0801121913aaf504e4bc1e62173f87a4378c37b49c8ccff196ce3f0ad20a3e0a0a626967446563696d616c12304a2e0a21121f06fcc6ae5fc3112150040ac90ef8cf52a9aafe8ce0940e7acd12cddbe9f80010f2ffffffffffffffff010a430a0d6e6567426967446563696d616c12324a300a230801121f06fcc6ae5fc3112150040ac90ef8cf52a9aafe8ce0940e7acd12cddbe9f80010f2ffffffffffffffff010a0e0a086973416374697665120228010a1a0a0474616773121252100a063204746167310a063204746167320a0f0a0464617461120722050102030405";
        let bytes = hex::decode(hex_bytes).unwrap();
        let buf = bytes.as_slice();

        let rich_struct = RichStruct::decode(buf).unwrap();
        let converted_entity: TestEntityWithBigTypesAndBytes =
            from_rich_struct(&rich_struct).unwrap();

        assert_eq!(converted_entity.id, "test");
        assert_eq!(converted_entity.name, "Test Entity");
        assert_eq!(
            converted_entity.big_int,
            BigInt::from_str("123456789012345678901234567890123456789012345678901234567890")
                .unwrap()
        );
        assert_eq!(
            converted_entity.neg_big_int,
            BigInt::from_str("-123456789012345678901234567890123456789012345678901234567890")
                .unwrap()
        );
        assert_eq!(
            converted_entity.big_decimal,
            BigDecimal::from_str(
                "123456789012345678901234567890123456789012345678901234567890.123"
            )
            .unwrap()
        );
        assert_eq!(
            converted_entity.neg_big_decimal,
            BigDecimal::from_str(
                "-123456789012345678901234567890123456789012345678901234567890.123"
            )
            .unwrap()
        );
        assert_eq!(converted_entity.is_active, true);
        assert_eq!(
            converted_entity.tags,
            vec!["tag1".to_string(), "tag2".to_string()]
        );
        assert_eq!(
            converted_entity.data,
            Bytes::from(vec![0x01, 0x02, 0x03, 0x04, 0x05])
        );
    }
}
