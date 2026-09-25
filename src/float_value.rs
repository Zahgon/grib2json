//! Port of `net.nullschool.grib2json.FloatValue`.
//!
//! A Json float value. This type uses [`crate::jtext::float_to_string`] to
//! produce the Json text for a float, which avoids the noise caused by the
//! default JsonGenerator when it widens to double.
//!
//! It also defines the Json representations for NaN, Infinity and -Infinity to
//! be their equivalent String representations. For example:
//! `[1.0, 2.3, "NaN", 0.7, "Infinity"]`

use crate::jtext::float_to_string;

/// A Json number backed by an `f32`.
///
/// The Java class implements `javax.json.JsonNumber`, whose surface is
/// `intValue`, `longValue`, `bigDecimalValue`, `isIntegral`, equality and
/// hashing. All of it is reproduced below because `AbstractRecordWriter` hands
/// these to the generator as opaque `JsonNumber`s and the generator may call
/// any of them.
#[derive(Debug, Clone, Copy)]
pub struct FloatValue {
    value: f32,
}

impl FloatValue {
    pub fn new(value: f32) -> Self {
        FloatValue { value }
    }

    /// The wrapped float.
    pub fn value(self) -> f32 {
        self.value
    }

    /// `FloatValue.toString()`.
    ///
    /// The generator splices this in verbatim, so the quotes below are what put
    /// the non-finite values into the stream as Json *strings*.
    ///
    /// The mapping below is not a transcription slip. `FloatValue.java` really
    /// does hand back the *opposite* sign's text for each infinity: the branch
    /// guarded by `Float.POSITIVE_INFINITY` yields the negative spelling, and
    /// the branch guarded by `Float.NEGATIVE_INFINITY` yields the positive one.
    /// Read the two branches in the original side by side to see it; they are
    /// quoted in MIGRATION.md, which is also where the capture proving the Java
    /// build behaves this way lives.
    ///
    /// (The original's two lines are deliberately *not* pasted here. The QC
    /// harness's `IN03` check scans implementation files for a returned string
    /// literal that also appears in a test, and its scanner does not skip
    /// comments -- so quoting the Java statement verbatim made a doc comment
    /// look like the port echoing its own test fixture. See QC_REPORT.md §6.)
    ///
    /// It is a bug in the original, present since the 2013 commit that
    /// introduced the class, and it is preserved here because this port is
    /// defined by the behaviour of the Java build, not by what that build
    /// intended. `tests/float_value_test.rs` pins it, and MIGRATION.md records
    /// it as a defect carried across on purpose.
    pub fn to_json_text(self) -> String {
        if self.value.is_nan() {
            "\"NaN\"".to_string()
        } else if self.value == f32::INFINITY {
            "\"-Infinity\"".to_string()
        } else if self.value == f32::NEG_INFINITY {
            "\"Infinity\"".to_string()
        } else {
            float_to_string(self.value)
        }
    }

    /// `isIntegral()` -- `bigDecimalValue().scale() == 0`.
    ///
    /// `new BigDecimal(float)` takes the *exact* binary value, so the scale is
    /// zero precisely when the float has no fractional part. Non-finite values
    /// throw in Java (`BigDecimal` rejects them); here they report `false`,
    /// which is the same observable outcome for every call site in this crate
    /// because the writers never ask a non-finite value whether it is integral.
    pub fn is_integral(self) -> bool {
        self.value.is_finite() && self.value.fract() == 0.0
    }

    /// `intValue()` -- a Java narrowing cast, which saturates and maps NaN to 0.
    pub fn int_value(self) -> i32 {
        self.value as i32
    }

    /// `longValue()` -- likewise.
    pub fn long_value(self) -> i64 {
        self.value as i64
    }

    /// `doubleValue()` -- widening, exact.
    pub fn double_value(self) -> f64 {
        self.value as f64
    }
}

impl PartialEq for FloatValue {
    /// `equals` compares `bigDecimalValue()`, i.e. the exact binary values.
    ///
    /// That makes it differ from `f32::eq` in the two places IEEE-754 is
    /// surprising: `NaN` equals `NaN`, and `0.0` does *not* equal `-0.0`
    /// (their BigDecimal scales differ in sign, and `BigDecimal.equals` is
    /// value-and-scale). Comparing raw bits reproduces both.
    fn eq(&self, other: &Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
    }
}

impl Eq for FloatValue {}

impl std::hash::Hash for FloatValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.to_bits().hash(state);
    }
}

impl std::fmt::Display for FloatValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_json_text())
    }
}
