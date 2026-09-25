//! The cross-implementation check: the Rust port and the Java original must
//! produce the same bytes for the same input.
//!
//! `tests/fixtures/sample.grib2` is a hand-built GRIB2 file (two messages, a
//! 4x3 lat/lon grid on template 3.0, simple packing on template 5.0 -- see
//! `tests/fixtures/make_sample_grib.py`). `tests/fixtures/java_names_data.json`
//! is what the Java build at commit `7fb455d7` printed for
//! `grib2json --names --data sample.grib2`, captured under
//! `maven:3.9-eclipse-temurin-17`.
//!
//! A same-language suite cannot catch a whole class of divergence -- key order,
//! indentation, the exact spelling of a code-table name, how a float is
//! rendered. Diffing against the original's real output can.

mod common;

use common::{fixture, run};
use grib2json::launcher::EXIT_OK;

/// Byte-for-byte agreement with the Java output for `--names --data`.
#[test]
fn matches_the_java_output_byte_for_byte() {
    assert_java_output("sample.grib2", "java_names_data.json");
}

/// The same check over the other four grid definition templates.
///
/// `templates.grib2` carries a rotated lat/lon grid (3.1), a Lambert conformal
/// grid (3.30) and a space-view grid (3.90), on earth shapes 1 and 3. Between
/// them they exercise every arm of `writeGridDefinition` and both arms of
/// `writeGridShape` -- and they are what caught the two unit scalings and the
/// IBM-format float that a spec-faithful reading gets wrong.
#[test]
fn matches_the_java_output_across_grid_templates() {
    assert_java_output("templates.grib2", "java_templates.json");
}

fn assert_java_output(input: &str, captured: &str) {
    let expected = std::fs::read_to_string(fixture(captured))
        .expect("the captured Java output is committed alongside the fixture");
    let path = fixture(input);

    let (status, actual, err) = run(&["--names", "--data", path.to_str().unwrap()]);
    assert_eq!(status, EXIT_OK, "stderr: {err}");

    if actual != expected {
        // Report the first divergence rather than dumping two documents.
        let (line, e, a) = expected
            .lines()
            .zip(actual.lines())
            .enumerate()
            .find(|(_, (e, a))| e != a)
            .map(|(i, (e, a))| (i + 1, e.to_string(), a.to_string()))
            .unwrap_or((0, format!("{} lines", expected.lines().count()), format!("{} lines", actual.lines().count())));
        panic!("diverged at line {line}\n  java: {e}\n  rust: {a}");
    }
}

/// The compact form is the same document with every byte of whitespace removed,
/// which is what `--compact` means in the Java (`PRETTY_PRINTING` unset).
#[test]
fn compact_output_carries_the_same_content_without_whitespace() {
    let path = fixture("sample.grib2");
    let (_, pretty, _) = run(&["--names", "--data", path.to_str().unwrap()]);
    let (status, compact, err) =
        run(&["--compact", "--names", "--data", path.to_str().unwrap()]);
    assert_eq!(status, EXIT_OK, "stderr: {err}");

    assert!(!compact.contains('\n'), "compact output must be one line");
    assert!(!compact.starts_with(' '), "compact output must not be indented");
    // Strip only the pretty form's *layout* -- the newline before each item and
    // the indent that follows it. Spaces inside string values are content
    // ("Meteorological products") and must survive.
    let stripped: String =
        pretty.lines().map(|l| l.trim_start()).collect::<Vec<_>>().concat();
    assert_eq!(compact, stripped);
}

/// Filters select the same records the Java filters select.
#[test]
fn filters_select_the_same_records() {
    let path = fixture("sample.grib2");
    let p = path.to_str().unwrap();

    // The fixture holds parameter 2 and parameter 3, both category 2.
    let (_, both, _) = run(&["--fp", "wind", p]);
    assert_eq!(both.matches("\"parameterNumber\"").count(), 2);

    let (_, only_u, _) = run(&["--fp", "2", p]);
    assert_eq!(only_u.matches("\"parameterNumber\"").count(), 1);
    assert!(only_u.contains("\"parameterNumber\":2"));

    let (_, none, _) = run(&["--fp", "7", p]);
    assert_eq!(none, "\n[\n]");

    // An unparsable filter rejects everything -- the Java catches
    // NumberFormatException and returns false.
    let (status, none, _) = run(&["--fp", "not-a-number", p]);
    assert_eq!(status, EXIT_OK);
    assert_eq!(none, "\n[\n]");

    // Discipline, category and surface filters.
    assert_eq!(run(&["--fd", "0", p]).1.matches("\"discipline\"").count(), 2);
    assert_eq!(run(&["--fd", "10", p]).1, "\n[\n]");
    assert_eq!(run(&["--fc", "2", p]).1.matches("\"parameterCategory\"").count(), 2);
    assert_eq!(run(&["--fc", "1", p]).1, "\n[\n]");
    assert_eq!(run(&["--fs", "103", p]).1.matches("\"surface1Type\"").count(), 2);
    assert_eq!(run(&["--fs", "100", p]).1, "\n[\n]");
}

/// A recipe file produces one document per line, with the command line
/// overriding the recipe because `merge` places it second.
#[test]
fn recipe_file_produces_one_document_per_line() {
    let dir = std::env::temp_dir().join("grib2json-recipe-test");
    std::fs::create_dir_all(&dir).unwrap();
    let recipe = dir.join("recipe.txt");
    std::fs::write(&recipe, "--fp 2\n--fp 3\n").unwrap();

    let path = fixture("sample.grib2");
    let (status, out, err) =
        run(&["--recipe", recipe.to_str().unwrap(), path.to_str().unwrap()]);
    assert_eq!(status, EXIT_OK, "stderr: {err}");

    // Two lines, so two concatenated Json documents, one record each.
    assert_eq!(out.matches("\"gribEdition\"").count(), 2);
    assert!(out.contains("\"parameterNumber\":2"));
    assert!(out.contains("\"parameterNumber\":3"));
    let _ = std::fs::remove_file(&recipe);
}

/// Without `--data` the `data` array is absent; with it, the values round-trip
/// through simple packing.
#[test]
fn data_array_appears_only_with_the_data_flag() {
    let path = fixture("sample.grib2");
    let p = path.to_str().unwrap();

    let (_, without, _) = run(&["--fp", "2", p]);
    assert!(!without.contains("\"data\""));

    let (_, with, _) = run(&["--data", "--fp", "2", p]);
    assert!(with.contains("\"data\""));
    // The values the fixture was built from, at the packing's 1/100 resolution.
    for expected in ["-2.12", "-2.27", "-2.41", "99.99", "-50.5", "0.01"] {
        assert!(with.contains(expected), "missing {expected} in data array");
    }
}
