//! Floating-point numbers over a wire that has no number for the three special values.
//!
//! JSON writes NaN and the infinities as `null` and refuses `null` back into an `f64`, so a model
//! carrying one could not cross a server function at all. They travel as their XSD lexemes —
//! `NaN`, `INF`, `-INF` — and every finite value stays a plain number.

use core::fmt;
use core::marker::PhantomData;

use serde::de::{
    self,
    Visitor,
};
use serde::{
    Deserializer,
    Serializer,
};

/// The lexemes `xs:double` and `xs:float` give the values JSON cannot spell.
fn lexeme(value: f64) -> &'static str {
    if value.is_nan() {
        return "NaN";
    }
    match value > 0.0 {
        true => "INF",
        false => "-INF",
    }
}

fn parse(text: &str) -> Option<f64> {
    match text.trim() {
        "INF" | "+INF" | "Infinity" => Some(f64::INFINITY),
        "-INF" | "-Infinity" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        number => number.parse().ok(),
    }
}

struct Real<T>(PhantomData<T>);

impl<T: From<f32> + FromF64> Visitor<'_> for Real<T> {
    type Value = T;

    fn expecting(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str("a number, or `NaN`, `INF` or `-INF` as a string")
    }

    fn visit_f64<E: de::Error>(
        self,
        value: f64,
    ) -> Result<T, E> {
        Ok(T::from_f64(value))
    }

    fn visit_f32<E: de::Error>(
        self,
        value: f32,
    ) -> Result<T, E> {
        Ok(T::from(value))
    }

    #[expect(clippy::cast_precision_loss, reason = "a whole number a format wrote without a point")]
    fn visit_i64<E: de::Error>(
        self,
        value: i64,
    ) -> Result<T, E> {
        Ok(T::from_f64(value as f64))
    }

    #[expect(clippy::cast_precision_loss, reason = "a whole number a format wrote without a point")]
    fn visit_u64<E: de::Error>(
        self,
        value: u64,
    ) -> Result<T, E> {
        Ok(T::from_f64(value as f64))
    }

    fn visit_str<E: de::Error>(
        self,
        text: &str,
    ) -> Result<T, E> {
        parse(text)
            .map(T::from_f64)
            .ok_or_else(|| E::custom(format!("`{text}` is not a floating-point number")))
    }
}

/// Narrowing to the type the model holds, which for `f32` the encoder widened losslessly.
trait FromF64 {
    fn from_f64(value: f64) -> Self;
}

impl FromF64 for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }
}

impl FromF64 for f32 {
    #[expect(clippy::cast_possible_truncation, reason = "the value was widened from an f32")]
    fn from_f64(value: f64) -> Self {
        value as Self
    }
}

/// `serde(with = …)` for an `f64` that may be NaN or infinite.
pub(crate) mod double {
    use super::{
        Deserializer,
        PhantomData,
        Real,
        Serializer,
        lexeme,
    };

    pub(crate) fn serialize<S: Serializer>(
        value: &f64,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value.is_finite() {
            true => serializer.serialize_f64(*value),
            false => serializer.serialize_str(lexeme(*value)),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        deserializer.deserialize_any(Real(PhantomData))
    }
}

/// `serde(with = …)` for an `f32` that may be NaN or infinite.
pub(crate) mod float {
    use super::{
        Deserializer,
        PhantomData,
        Real,
        Serializer,
        lexeme,
    };

    pub(crate) fn serialize<S: Serializer>(
        value: &f32,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value.is_finite() {
            true => serializer.serialize_f32(*value),
            false => serializer.serialize_str(lexeme(f64::from(*value))),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
        deserializer.deserialize_any(Real(PhantomData))
    }
}
