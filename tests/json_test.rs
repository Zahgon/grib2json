//! The `javax.json` generator behaviour, and the `Float`/`Double`/joda text
//! formatting under it.
//!
//! Every expectation here is a capture from `org.glassfish:javax.json:1.0.3`
//! and JDK 17, not a guess about what pretty JSON ought to look like.

use grib2json::float_value::FloatValue;
use grib2json::json::to_string;
use grib2json::jtext::{double_to_string, float_to_string, DateTime};

/// The pretty printer's exact layout, captured from the Java generator.
///
/// Three details a reimplementation gets wrong by default: the document opens
/// with a newline, there is no space after `:`, and an empty structure still
/// spans two lines.
#[test]
fn pretty_layout_matches_the_java_generator() {
    let actual = to_string(true, |jg| {
        jg.write_start_array()?;
        jg.write_start_object()?;
        jg.write_start_object_named("header")?;
        jg.write_int("discipline", 0)?;
        jg.write_string("disciplineName", "Meteorological products")?;
        jg.write_double("surface1Value", 10.0)?;
        jg.write_long("gribLength", 27759)?;
        jg.write_end()?;
        jg.write_start_array_named("data")?;
        jg.write_float_value_item(FloatValue::new(-2.12))?;
        jg.write_float_value_item(FloatValue::new(-2.27))?;
        jg.write_end()?;
        jg.write_end()?;
        jg.write_start_object()?;
        jg.write_end()?;
        jg.write_start_array()?;
        jg.write_end()?;
        jg.write_end()
    })
    .unwrap();

    let expected = "\n[\n    {\n        \"header\":{\n            \"discipline\":0,\n            \
        \"disciplineName\":\"Meteorological products\",\n            \"surface1Value\":10.0,\n            \
        \"gribLength\":27759\n        },\n        \"data\":[\n            -2.12,\n            -2.27\n        \
        ]\n    },\n    {\n    },\n    [\n    ]\n]";
    assert_eq!(actual, expected);
}

/// Compact mode emits no whitespace at all, and `JsonValue.NULL` is `null`.
#[test]
fn compact_layout_matches_the_java_generator() {
    let actual = to_string(false, |jg| {
        jg.write_start_array()?;
        jg.write_start_object()?;
        jg.write_int("a", 1)?;
        jg.write_start_array_named("d")?;
        jg.write_float_value_item(FloatValue::new(1.5))?;
        jg.write_null_item()?;
        jg.write_end()?;
        jg.write_end()?;
        jg.write_end()
    })
    .unwrap();
    assert_eq!(actual, "[{\"a\":1,\"d\":[1.5,null]}]");
}

/// Escaping: five short escapes, `\uXXXX` for the remaining C0 controls, and
/// non-ASCII written through raw rather than as surrogate escapes.
#[test]
fn string_escaping_matches_the_java_generator() {
    let actual = to_string(false, |jg| {
        jg.write_start_object()?;
        jg.write_string(
            "k",
            "quote\" back\\ tab\t nl\n cr\r bs\u{8} ff\u{c} us\u{1f} tilde~ eacute\u{e9} cjk\u{4e2d}",
        )?;
        jg.write_end()
    })
    .unwrap();
    assert_eq!(
        actual,
        "{\"k\":\"quote\\\" back\\\\ tab\\t nl\\n cr\\r bs\\b ff\\f us\\u001f tilde~ eacuteé cjk中\"}"
    );
}

/// The non-finite `FloatValue`s reach the stream as Json strings.
#[test]
fn non_finite_floats_reach_the_stream_as_strings() {
    let actual = to_string(true, |jg| {
        jg.write_start_array()?;
        jg.write_float_value_item(FloatValue::new(f32::NAN))?;
        jg.write_float_value_item(FloatValue::new(f32::INFINITY))?;
        jg.write_float_value_item(FloatValue::new(f32::NEG_INFINITY))?;
        jg.write_end()
    })
    .unwrap();
    assert_eq!(actual, "\n[\n    \"NaN\",\n    \"-Infinity\",\n    \"Infinity\"\n]");
}

/// `Double.toString`, captured from JDK 17. Same thresholds as `Float`.
#[test]
fn double_to_string_matches_java() {
    let cases: &[(f64, &str)] = &[
        (0.0, "0.0"),
        (10.0, "10.0"),
        (1.0 / 3.0, "0.3333333333333333"),
        ((20 + 359) as f64 + 2.0 / 3.0, "379.6666666666667"),
        (-80.0, "-80.0"),
        (0.5, "0.5"),
        (1e7, "1.0E7"),
        (1e-3, "0.001"),
        (1e-4, "1.0E-4"),
        (100.0, "100.0"),
        (20.0, "20.0"),
    ];
    for (value, expected) in cases {
        assert_eq!(double_to_string(*value), *expected, "double {value:?}");
    }
}

/// `float_to_string` and `double_to_string` agree wherever the value is exactly
/// representable in binary.
///
/// Only exact values are compared. `0.001f32` widened to `f64` is
/// `0.0010000000474974513`, and printing that is correct for both languages --
/// `Double.toString((double) 0.001f)` says the same thing. The divergence is
/// the widening, which is precisely what `FloatValue` exists to keep out of the
/// output.
#[test]
fn float_and_double_agree_on_exactly_representable_values() {
    for v in [1.0f32, 0.5, 0.25, 100.0, 1000.0, -2.5, 0.0] {
        assert_eq!(float_to_string(v), double_to_string(v as f64), "value {v}");
    }
    // And the widening case the class was written to avoid.
    assert_eq!(float_to_string(0.001), "0.001");
    assert_eq!(double_to_string(0.001f32 as f64), "0.0010000000474974513");
}

/// joda's `DateTime.toString()` in UTC always prints milliseconds and a literal `Z`.
#[test]
fn iso_timestamps_match_joda() {
    // The reference time in the committed GRIB fixture.
    assert_eq!(
        DateTime::from_utc(2013, 10, 24, 18, 0).to_iso_string(),
        "2013-10-24T18:00:00.000Z"
    );
    // The Unix epoch, and the OSCAR base date the NetCDF branch counts from.
    assert_eq!(DateTime::from_millis(0).to_iso_string(), "1970-01-01T00:00:00.000Z");
    assert_eq!(
        DateTime::from_utc(1992, 10, 5, 0, 0).to_iso_string(),
        "1992-10-05T00:00:00.000Z"
    );
    // plusDays across a leap day.
    assert_eq!(
        DateTime::from_utc(2020, 2, 28, 12, 0).plus_days(1).to_iso_string(),
        "2020-02-29T12:00:00.000Z"
    );
    assert_eq!(
        DateTime::from_utc(2019, 2, 28, 12, 0).plus_days(1).to_iso_string(),
        "2019-03-01T12:00:00.000Z"
    );
    // A pre-epoch instant, which needs floor division rather than truncation.
    assert_eq!(
        DateTime::from_utc(1969, 12, 31, 23, 59).to_iso_string(),
        "1969-12-31T23:59:00.000Z"
    );
    // Sub-second precision survives the round trip.
    assert_eq!(DateTime::from_millis(1_384_000_123).to_iso_string(), "1970-01-17T00:26:40.123Z");
}
