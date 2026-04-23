/// Scalar value flowing through the pipeline.
///
/// `Pulse` is a momentary true that auto-resets to false after one tick.
/// `Null` means "no value this tick" (suppressed output).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Pulse,
    Int(i64),
    Float(f64),
    Null,
}

/// The type tag used in device definitions and node port declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    Bool,
    Pulse,
    Int,
    Float,
}

/// Range constraint for `Int` and `Float` values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
}

/// What to do when a value falls outside the declared range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
}
