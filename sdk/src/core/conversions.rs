use anyhow::{anyhow, Result};

use crate::common::{BigDecimal as ProtoBigDecimal, BigInteger as ProtoBigInteger};
use crate::entity::types::{BigDecimal, BigInt};

// Centralized numeric <-> protobuf conversions shared across the crate

pub fn bigint_to_proto(value: &BigInt) -> ProtoBigInteger {
    use num_bigint::Sign;
    let (sign, bytes) = value.to_bytes_be();
    ProtoBigInteger { negative: sign == Sign::Minus, data: bytes }
}

pub fn proto_to_bigint(proto: &ProtoBigInteger) -> BigInt {
    use num_bigint::Sign;
    let sign = if proto.negative { Sign::Minus } else { Sign::Plus };
    BigInt::from_bytes_be(sign, &proto.data)
}

pub fn bigdecimal_to_proto(value: &BigDecimal) -> ProtoBigDecimal {
    let (mantissa_bigint, scale) = value.as_bigint_and_exponent();
    let proto_mantissa = bigint_to_proto(&mantissa_bigint);
    ProtoBigDecimal { value: Some(proto_mantissa), exp: scale as i32 }
}

pub fn proto_to_bigdecimal(proto: &ProtoBigDecimal) -> Result<BigDecimal> {
    let mantissa = proto
        .value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing mantissa in BigDecimal"))?;
    let mantissa_bigint = proto_to_bigint(mantissa);
    let exponent = proto.exp as i64;
    let scale = -exponent;
    Ok(BigDecimal::new(mantissa_bigint, scale))
}

