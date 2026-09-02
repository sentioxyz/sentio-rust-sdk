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
    // bigdecimal: value = mantissa * 10^(-scale); proto (and the Go side's
    // decimal.NewFromBigInt): value = mantissa * 10^exp. So exp = -scale.
    let (mantissa_bigint, scale) = value.as_bigint_and_exponent();
    let proto_mantissa = bigint_to_proto(&mantissa_bigint);
    ProtoBigDecimal { value: Some(proto_mantissa), exp: -(scale as i32) }
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigdecimal_round_trips_and_uses_go_exponent_semantics() {
        for text in ["1.5", "622080000.000000000000000001", "-0.001", "1000000000000000000", "1E+30", "0"] {
            let value: BigDecimal = text.parse().unwrap();
            let proto = bigdecimal_to_proto(&value);
            assert_eq!(proto_to_bigdecimal(&proto).unwrap(), value, "{}", text);
        }
        // The platform decodes value = mantissa * 10^exp, so 1.5 is (15, -1).
        let proto = bigdecimal_to_proto(&"1.5".parse().unwrap());
        assert_eq!(proto.exp, -1);
        assert_eq!(proto_to_bigint(proto.value.as_ref().unwrap()).to_string(), "15");
    }
}
