//! Migrated from `src/test/java/net/nullschool/grib2json/FloatValueTest.java`.
//!
//! # What the Java test contained
//!
//! ```java
//! public class FloatValueTest {
//!     @Test
//!     public void test() {
//!     }
//! }
//! ```
//!
//! The body is empty. It declares one test, asserts nothing, and executes none
//! of `FloatValue`. Surefire counts it as a pass because JUnit passes any method
//! that returns without throwing.
//!
//! # What was migrated
//!
//! `test` is kept under its own name so the test inventories reconcile
//! one-for-one. Transcribing the empty body would have reproduced the *count*
//! and none of the *coverage*, and a suite that asserts nothing cannot show that
//! this port behaves like the original -- which is the whole claim being made.
//! So `test` carries the contract the empty method left unstated, and the cases
//! around it pin the behaviours captured from the Java build.
//!
//! Every expected value below was produced by running Java, not by reading the
//! Rust. The captures are listed in MIGRATION.md.

use grib2json::float_value::FloatValue;

/// Counterpart of `FloatValueTest.test()`.
#[test]
fn test() {
    // The class exists to render a float as Json text without the widening
    // noise a double-based generator introduces.
    assert_eq!(FloatValue::new(1.0).to_json_text(), "1.0");
    assert_eq!(FloatValue::new(-2.12).to_json_text(), "-2.12");
    assert_eq!(FloatValue::new(0.0).to_json_text(), "0.0");
}

/// `Float.toString` chooses plain decimal inside `[1e-3, 1e7)` and scientific
/// notation outside it. Values captured from JDK 17.
#[test]
fn renders_the_same_text_java_renders() {
    let cases: &[(f32, &str)] = &[
        (0.0, "0.0"),
        (-0.0, "-0.0"),
        (1.0, "1.0"),
        (-1.0, "-1.0"),
        (0.5, "0.5"),
        (1.5, "1.5"),
        (-2.12, "-2.12"),
        (-2.27, "-2.27"),
        (-2.41, "-2.41"),
        (1e-3, "0.001"),
        (9.999e-4, "9.999E-4"),
        (1e-4, "1.0E-4"),
        (1.0e7, "1.0E7"),
        (9999999.0, "9999999.0"),
        (1.0e8, "1.0E8"),
        (1.23456e-5, "1.23456E-5"),
        (3.4028235e38, "3.4028235E38"),
        (0.1, "0.1"),
        (0.2, "0.2"),
        (0.3, "0.3"),
        (100.0, "100.0"),
        (1000.0, "1000.0"),
        (123.456, "123.456"),
        (1e20, "1.0E20"),
        (-1e-20, "-1.0E-20"),
        (2.5, "2.5"),
        (1.0 / 3.0, "0.33333334"),
        (10.0 / 3.0, "3.3333333"),
    ];
    for (value, expected) in cases {
        assert_eq!(
            FloatValue::new(*value).to_json_text(),
            *expected,
            "float {value:?} (bits {:#x})",
            value.to_bits()
        );
    }
}

/// NaN and both infinities become Json *strings*, and the two infinities are
/// swapped.
///
/// This is a defect in the original, carried across deliberately:
///
/// ```java
/// else if (value == Float.POSITIVE_INFINITY) { return "\"-Infinity\""; }
/// else if (value == Float.NEGATIVE_INFINITY) { return "\"Infinity\"";  }
/// ```
///
/// A port that "fixed" it would produce different bytes than the program being
/// replaced. Confirmed against the Java build, which emits
/// `["NaN","-Infinity","Infinity"]` for `[NaN, +inf, -inf]`.
#[test]
fn non_finite_values_are_quoted_and_the_infinities_are_swapped() {
    assert_eq!(FloatValue::new(f32::NAN).to_json_text(), "\"NaN\"");
    assert_eq!(FloatValue::new(f32::INFINITY).to_json_text(), "\"-Infinity\"");
    assert_eq!(FloatValue::new(f32::NEG_INFINITY).to_json_text(), "\"Infinity\"");
}

/// `isIntegral()` is `bigDecimalValue().scale() == 0`, i.e. no fractional part.
#[test]
fn reports_whether_the_value_is_integral() {
    assert!(FloatValue::new(1.0).is_integral());
    assert!(FloatValue::new(-7.0).is_integral());
    assert!(FloatValue::new(0.0).is_integral());
    assert!(!FloatValue::new(1.5).is_integral());
    assert!(!FloatValue::new(-0.25).is_integral());
}

/// The narrowing accessors are Java casts: truncation toward zero.
#[test]
fn narrowing_accessors_truncate_toward_zero() {
    assert_eq!(FloatValue::new(2.9).int_value(), 2);
    assert_eq!(FloatValue::new(-2.9).int_value(), -2);
    assert_eq!(FloatValue::new(2.9).long_value(), 2);
    assert_eq!(FloatValue::new(-2.9).long_value(), -2);
    // Widening is exact.
    assert_eq!(FloatValue::new(0.5).double_value(), 0.5_f64);
}

/// `equals` compares exact values, so it differs from `==` on floats in the two
/// places IEEE-754 is surprising.
#[test]
fn equality_follows_big_decimal_not_ieee() {
    assert_eq!(FloatValue::new(f32::NAN), FloatValue::new(f32::NAN));
    assert_ne!(FloatValue::new(0.0), FloatValue::new(-0.0));
    assert_eq!(FloatValue::new(1.25), FloatValue::new(1.25));
}
