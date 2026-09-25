//! Migrated from `src/test/java/net/nullschool/grib2json/LauncherTest.java`.
//!
//! # What the Java test contained
//!
//! ```java
//! public class LauncherTest {
//!     @Test
//!     public void test_1() {
//! //        Launcher.main(new String[] {"c:/users/cambecc/desktop/gfs/gfs.t18z.pgrb2f00", "out.txt", "true"});
//! //        Launcher.main(new String[] {"c:/users/cambecc/desktop/gfs/gfs.t18z.pgrbf00.2p5deg.grib2", "out.txt", "false"});
//! //        String args = "--fc 2 --fs 103 --fv 80 --names c:/users/cambecc/desktop/gfs/gfs.t18z.pgrbf00.2p5deg.grib2";
//! //        Launcher.main(args.split(" "));
//!     }
//! }
//! ```
//!
//! Every line is commented out, and the paths are on a developer's Windows
//! desktop, so the test could never have run anywhere else. It asserts nothing
//! and executes nothing.
//!
//! # What was migrated
//!
//! `test_1` keeps its name, and its body does what the commented-out lines were
//! reaching for: drive `Launcher` end to end, including the third invocation's
//! `--fc 2 --fs 103 --fv 80 --names` filter, against a fixture committed to this
//! repository instead of an absent desktop file. The remaining cases pin the
//! launcher behaviour captured from the Java build -- exit statuses, which
//! stream each message lands on, and the exact `--help` text.

mod common;

use common::{fixture, run};
use grib2json::launcher::{EXIT_ERROR, EXIT_OK, EXIT_USAGE};
use grib2json::options::help_message;

/// Counterpart of `LauncherTest.test_1()`.
#[test]
fn test_1() {
    let path = fixture("sample.grib2");
    let path = path.to_str().unwrap();

    // The first two commented-out invocations: run the converter over a file.
    let (status, out, err) = run(&[path]);
    assert_eq!(status, EXIT_OK, "stderr: {err}");
    assert!(err.is_empty(), "expected a clean run, got: {err}");
    assert!(out.starts_with("\n[\n    {\n"), "unexpected start: {:?}", &out[..40.min(out.len())]);
    assert!(out.trim_end().ends_with(']'));

    // The third: a filtered run with names printed. Surface type 103 is present
    // in the fixture but its value is 10.0, so `--fv 80` selects nothing and the
    // document is an empty array.
    let (status, out, err) =
        run(&["--fc", "2", "--fs", "103", "--fv", "80", "--names", path]);
    assert_eq!(status, EXIT_OK, "stderr: {err}");
    assert_eq!(out, "\n[\n]");

    // The same filter with the value the fixture actually carries selects both
    // records and prints the human-readable names.
    let (status, out, _) =
        run(&["--fc", "2", "--fs", "103", "--fv", "10", "--names", path]);
    assert_eq!(status, EXIT_OK);
    assert!(out.contains("\"parameterNumberName\":\"U-component_of_wind\""));
    assert!(out.contains("\"parameterNumberName\":\"V-component_of_wind\""));
}

/// `--help` prints usage on stdout and exits 0.
#[test]
fn help_exits_zero_and_prints_usage_on_stdout() {
    for flag in ["--help", "-h"] {
        let (status, out, err) = run(&[flag]);
        assert_eq!(status, EXIT_OK, "{flag}");
        assert_eq!(out, format!("{}\n", help_message()), "{flag}");
        assert!(err.is_empty(), "{flag}");
    }
}

/// No FILE prints the same usage but exits 1, because `getFile()` is null.
#[test]
fn missing_file_prints_usage_and_exits_one() {
    let (status, out, err) = run(&[]);
    assert_eq!(status, EXIT_USAGE);
    assert_eq!(out, format!("{}\n", help_message()));
    assert!(err.is_empty());
}

/// An unknown option prints usage, then a blank line, then the message on stderr.
///
/// The blank line is `System.out.println()` between `printUsage()` and the
/// message -- captured from the Java build.
#[test]
fn unknown_option_prints_usage_then_blank_line_then_stderr() {
    let (status, out, err) = run(&["--bogus"]);
    assert_eq!(status, EXIT_USAGE);
    assert_eq!(out, format!("{}\n\n", help_message()));
    assert_eq!(err, "Unexpected Option: bogus\n");
}

/// A value option given without a value quotes its own spec line back.
#[test]
fn missing_option_value_quotes_the_spec_line() {
    let (status, _, err) = run(&["--fp"]);
    assert_eq!(status, EXIT_USAGE);
    assert_eq!(
        err,
        "Option must have a value: [--filter.parameter --fp value] : select records \
         with this numeric parameter, or the string \"wind\" for both u,v components\n"
    );
}

/// An unparsable number reports once per long name, so an option with two long
/// names prints the same line twice. Captured from the Java build.
#[test]
fn unparsable_number_reports_once_per_long_name() {
    let (status, _, err) = run(&["--fc", "abc"]);
    assert_eq!(status, EXIT_USAGE);
    let line = "Invalid value (Unsupported number format: For input string: \"abc\"): \
                [--filter.category --fc value] : select records with this numeric category";
    assert_eq!(err, format!("{line}\n{line}\n"));
}

/// A missing input file is an `IllegalArgumentException`: bare stderr, exit 1,
/// and no usage text.
#[test]
fn missing_input_file_reports_and_exits_one() {
    let (status, out, err) = run(&["/nonexistent.grib2"]);
    assert_eq!(status, EXIT_USAGE);
    assert!(out.is_empty(), "no usage should be printed, got {out:?}");
    assert_eq!(err, "Cannot find input file: /nonexistent.grib2\n");
}

/// A file that is neither GRIB nor classic NetCDF falls through to the NetCDF
/// branch and fails there, which is the `catch (Throwable)` arm: exit 2.
#[test]
fn unreadable_format_exits_two() {
    let dir = std::env::temp_dir().join("grib2json-not-grib");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("junk.bin");
    std::fs::write(&path, b"this is not a GRIB message").unwrap();

    let (status, _, err) = run(&[path.to_str().unwrap()]);
    assert_eq!(status, EXIT_ERROR);
    assert!(err.contains("NetCDF"), "expected a NetCDF diagnosis, got: {err}");
    let _ = std::fs::remove_file(&path);
}

/// `--output` writes the document to a file and leaves stdout empty.
#[test]
fn output_option_writes_to_a_file() {
    let dir = std::env::temp_dir().join("grib2json-output-test");
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("out.json");
    let path = fixture("sample.grib2");

    let (status, out, err) =
        run(&["--output", target.to_str().unwrap(), path.to_str().unwrap()]);
    assert_eq!(status, EXIT_OK, "stderr: {err}");
    assert!(out.is_empty(), "stdout should be empty, got {out:?}");

    let written = std::fs::read_to_string(&target).unwrap();
    assert!(written.starts_with("\n[\n"));
    assert!(written.contains("\"gribEdition\":2"));
    let _ = std::fs::remove_file(&target);
}
