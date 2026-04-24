use serde::{Deserialize, Serialize};

/// Scalar value flowing through the pipeline.
///
/// `Pulse` is a momentary true that auto-resets to false after one tick.
/// `Null` means "no value this tick" (suppressed output).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Pulse,
    Int(i64),
    Float(f64),
    Null,
}

/// The type tag used in device definitions and node port declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueType {
    Bool,
    Pulse,
    Int,
    Float,
}

/// Range constraint for `Int` and `Float` values.
///
/// Invariant: `min <= max` and neither is NaN. Construct via [`ValueRange::new`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ValueRange {
    min: f64,
    max: f64,
}

impl ValueRange {
    /// Returns `Err` if `min > max` or either is NaN.
    pub fn new(min: f64, max: f64) -> Result<Self, &'static str> {
        if min.is_nan() || max.is_nan() || min > max {
            return Err("min must be <= max and neither NaN");
        }
        Ok(Self { min, max })
    }

    #[must_use]
    pub fn min(&self) -> f64 {
        self.min
    }

    #[must_use]
    pub fn max(&self) -> f64 {
        self.max
    }
}

/// What to do when a value falls outside the declared range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OutOfRange {
    #[default]
    Clamp,
    Ignore,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_clone() {
        let v = Value::Float(0.5);
        assert_eq!(v.clone(), Value::Float(0.5));
    }

    #[test]
    fn value_range_valid() {
        let r = ValueRange::new(0.0, 1.0).unwrap();
        assert!((r.min() - 0.0).abs() < f64::EPSILON);
        assert!((r.max() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn value_range_invalid() {
        assert!(ValueRange::new(1.0, 0.0).is_err());
        assert!(ValueRange::new(f64::NAN, 1.0).is_err());
    }
}
