//! The jewelcli-proxied `Options` interface: the grammar, the help text and the
//! parse errors.

use grib2json::options::{help_message, parse_arguments};

/// The `--help` text, byte for byte, as printed by the Java build.
///
/// Order comes from `OptionOrder.LONGNAME` on the `@CommandLineInterface`
/// annotation, the indent is a tab, and the spec lines are the same strings
/// jewelcli quotes back in an error message.
#[test]
fn help_message_matches_the_java_build() {
    let expected = "Usage: grib2json [options] FILE\n\
        \t[--compact -c] : enable compact Json formatting\n\
        \t[--data -d] : print GRIB record data\n\
        \t[--filter.category --fc value] : select records with this numeric category\n\
        \t[--filter.discipline --fd value] : select records with this discipline\n\
        \t[--filter.parameter --fp value] : select records with this numeric parameter, or the string \"wind\" for both u,v components\n\
        \t[--filter.surface --fs value] : select records with this numeric surface type\n\
        \t[--filter.value --fv value] : select records with this numeric surface value\n\
        \t[--help -h] : display this help\n\
        \t[--names -n] : print names of numeric codes\n\
        \t[--output -o value] : write output to the specified file (default is stdout)\n\
        \t[--recipe -r value] : a file containing a batch of filter options: fd, fc, fp, fs, fv, and o\n\
        \t[--verbose -v] : enable logging to stdout";
    assert_eq!(help_message(), expected);
}

/// Flags set their booleans and consume no value; long and short names agree.
#[test]
fn flags_parse_under_both_names() {
    let long = parse_arguments(&["--compact", "--data", "--names", "--verbose", "--help"]).unwrap();
    assert!(long.compact_format && long.print_data && long.print_names);
    assert!(long.enable_logging && long.show_help);

    let short = parse_arguments(&["-c", "-d", "-n", "-v", "-h"]).unwrap();
    assert_eq!(long, short);
}

/// The unparsed argument becomes FILE.
#[test]
fn unparsed_argument_becomes_the_file() {
    let o = parse_arguments(&["--names", "some.grib2"]).unwrap();
    assert_eq!(o.file.unwrap().to_str().unwrap(), "some.grib2");
    assert!(o.print_names);
}

/// Value options accept both `--name value` and `--name=value`.
#[test]
fn value_options_accept_both_forms() {
    let spaced = parse_arguments(&["--filter.category", "2", "--fv", "10.5", "-o", "x.json"]).unwrap();
    let equals = parse_arguments(&["--filter.category=2", "--fv=10.5", "--output=x.json"]).unwrap();
    assert_eq!(spaced.filter_category, Some(2));
    assert_eq!(spaced.filter_value, Some(10.5));
    assert_eq!(spaced.output.as_deref().unwrap().to_str().unwrap(), "x.json");
    assert_eq!(spaced, equals);
}

/// Every alias reaches the same field.
#[test]
fn aliases_reach_the_same_fields() {
    let short = parse_arguments(&["--fd", "0", "--fc", "2", "--fp", "wind", "--fs", "103", "--fv", "10"]).unwrap();
    let long = parse_arguments(&[
        "--filter.discipline", "0", "--filter.category", "2",
        "--filter.parameter", "wind", "--filter.surface", "103", "--filter.value", "10",
    ])
    .unwrap();
    assert_eq!(short, long);
    assert_eq!(short.filter_discipline, Some(0));
    assert_eq!(short.filter_parameter.as_deref(), Some("wind"));
}

/// Later occurrences win, which is what makes `Launcher.merge` place the recipe
/// first and the real command line second.
#[test]
fn later_occurrences_win() {
    let o = parse_arguments(&["--fp", "2", "--fp", "3"]).unwrap();
    assert_eq!(o.filter_parameter.as_deref(), Some("3"));
}

/// Absent options stay absent -- `defaultToNull=true` on every one of them.
#[test]
fn absent_options_are_none() {
    let o = parse_arguments(&["file.grib2"]).unwrap();
    assert_eq!(o.filter_discipline, None);
    assert_eq!(o.filter_category, None);
    assert_eq!(o.filter_parameter, None);
    assert_eq!(o.filter_surface, None);
    assert_eq!(o.filter_value, None);
    assert_eq!(o.recipe, None);
    assert_eq!(o.output, None);
    assert!(!o.show_help && !o.print_data && !o.print_names);
}

/// An unknown option is `Unexpected Option: <name>`.
#[test]
fn unknown_option_is_rejected() {
    let e = parse_arguments(&["--bogus"]).unwrap_err();
    assert_eq!(e.message, "Unexpected Option: bogus");
}

/// A value option at the end of the line has no value to take.
#[test]
fn value_option_without_a_value_is_rejected() {
    let e = parse_arguments(&["--fp"]).unwrap_err();
    assert!(e.message.starts_with("Option must have a value: [--filter.parameter --fp value]"));

    // A following option is not swallowed as the value either.
    let e = parse_arguments(&["--fc", "--names"]).unwrap_err();
    assert!(e.message.starts_with("Option must have a value: [--filter.category --fc value]"));
}

/// An unparsable number reports once per long name.
#[test]
fn unparsable_numbers_are_rejected_once_per_long_name() {
    let e = parse_arguments(&["--fc", "abc"]).unwrap_err();
    assert_eq!(e.message.lines().count(), 2, "two long names, two reports");
    assert!(e.message.lines().all(|l| l.starts_with(
        "Invalid value (Unsupported number format: For input string: \"abc\"): [--filter.category --fc value]"
    )));

    let e = parse_arguments(&["--fv", "xyz"]).unwrap_err();
    assert_eq!(e.message.lines().count(), 2);
    assert!(e.message.contains("For input string: \"xyz\""));
}

/// `--help` wins even when a FILE is also given -- the Java checks `getShowHelp()`
/// before it looks at `getFile()`.
#[test]
fn help_takes_precedence_over_the_file() {
    let o = parse_arguments(&["--help", "extra.grib2"]).unwrap();
    assert!(o.show_help);
    assert!(o.file.is_some());
}
