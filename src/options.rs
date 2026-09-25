//! Port of `net.nullschool.grib2json.Options` and the slice of
//! `com.lexicalscope.jewel.cli` that makes it observable.
//!
//! In Java, `Options` is an interface that jewelcli proxies; the annotations on
//! it are simultaneously the parser's grammar, the `--help` text, and the text
//! quoted back in a parse error. All three are reproduced here from output
//! captured off the Java build -- see `tests/options_test.rs`, which pins the
//! help message byte for byte.

use std::fmt;
use std::path::PathBuf;

/// The kind of value an option carries, which decides whether the spec line
/// ends in ` value` and whether the parser consumes the next argument.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arity {
    Flag,
    Value,
}

/// One entry of the options grammar.
struct Spec {
    /// Long names, in the order jewelcli prints them.
    long: &'static [&'static str],
    short: Option<&'static str>,
    description: &'static str,
    arity: Arity,
}

impl Spec {
    /// The bracketed form jewelcli prints in `--help` and quotes in errors:
    /// `[--filter.category --fc value] : select records with this numeric category`
    fn line(&self) -> String {
        let mut s = String::from("[");
        let mut parts: Vec<String> = self.long.iter().map(|l| format!("--{}", l)).collect();
        if let Some(short) = self.short {
            parts.push(format!("-{}", short));
        }
        s.push_str(&parts.join(" "));
        if self.arity == Arity::Value {
            s.push_str(" value");
        }
        s.push_str("] : ");
        s.push_str(self.description);
        s
    }
}

/// The grammar, ordered by long name -- `OptionOrder.LONGNAME` in the
/// `@CommandLineInterface` annotation, which is what fixes the `--help` order.
const SPECS: &[Spec] = &[
    Spec { long: &["compact"], short: Some("c"), description: "enable compact Json formatting", arity: Arity::Flag },
    Spec { long: &["data"], short: Some("d"), description: "print GRIB record data", arity: Arity::Flag },
    Spec { long: &["filter.category", "fc"], short: None, description: "select records with this numeric category", arity: Arity::Value },
    Spec { long: &["filter.discipline", "fd"], short: None, description: "select records with this discipline", arity: Arity::Value },
    Spec { long: &["filter.parameter", "fp"], short: None, description: "select records with this numeric parameter, or the string \"wind\" for both u,v components", arity: Arity::Value },
    Spec { long: &["filter.surface", "fs"], short: None, description: "select records with this numeric surface type", arity: Arity::Value },
    Spec { long: &["filter.value", "fv"], short: None, description: "select records with this numeric surface value", arity: Arity::Value },
    Spec { long: &["help"], short: Some("h"), description: "display this help", arity: Arity::Flag },
    Spec { long: &["names"], short: Some("n"), description: "print names of numeric codes", arity: Arity::Flag },
    Spec { long: &["output"], short: Some("o"), description: "write output to the specified file (default is stdout)", arity: Arity::Value },
    Spec { long: &["recipe"], short: Some("r"), description: "a file containing a batch of filter options: fd, fc, fp, fs, fv, and o", arity: Arity::Value },
    Spec { long: &["verbose"], short: Some("v"), description: "enable logging to stdout", arity: Arity::Flag },
];

/// `CliFactory.createCli(Options.class).getHelpMessage()`.
pub fn help_message() -> String {
    let mut s = String::from("Usage: grib2json [options] FILE");
    for spec in SPECS {
        s.push_str("\n\t");
        s.push_str(&spec.line());
    }
    s
}

/// `com.lexicalscope.jewel.JewelRuntimeException` -- the parse failures the
/// `Launcher` catches and turns into usage output plus a message on stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JewelRuntimeException {
    /// The message, exactly as jewelcli composes it. Multi-problem messages are
    /// newline-joined, which is why an unparsable number prints twice: jewelcli
    /// records one validation failure per long name of the offending option.
    pub message: String,
}

impl fmt::Display for JewelRuntimeException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// The parsed command line. Field-for-field with the Java interface's getters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    pub show_help: bool,
    pub print_names: bool,
    pub print_data: bool,
    pub compact_format: bool,
    pub enable_logging: bool,
    pub output: Option<PathBuf>,
    pub file: Option<PathBuf>,
    pub filter_discipline: Option<i32>,
    pub filter_category: Option<i32>,
    pub filter_parameter: Option<String>,
    pub filter_surface: Option<i32>,
    pub filter_value: Option<f64>,
    pub recipe: Option<PathBuf>,
}

/// Locate a spec by the name as written on the command line.
fn find(name: &str, long: bool) -> Option<&'static Spec> {
    SPECS.iter().find(|s| {
        if long {
            s.long.contains(&name)
        } else {
            s.short == Some(name)
        }
    })
}

/// `Integer.parseInt` as jewelcli reports it failing.
fn parse_int(raw: &str, spec: &Spec) -> Result<i32, JewelRuntimeException> {
    raw.parse::<i32>().map_err(|_| invalid_value(raw, spec))
}

/// `Double.parseDouble` as jewelcli reports it failing.
fn parse_double(raw: &str, spec: &Spec) -> Result<f64, JewelRuntimeException> {
    // Java accepts a leading/trailing space and the suffixes d/f; Rust's parser
    // does not. Trimming matches `Double.parseDouble`, which trims before parsing.
    raw.trim().parse::<f64>().map_err(|_| invalid_value(raw, spec))
}

/// The `Invalid value (...)` message, repeated once per long name.
///
/// jewelcli validates the value against each name the option answers to, so an
/// option with two long names reports the same problem twice. `--fc abc`
/// therefore puts two identical lines on stderr, and this reproduces that.
fn invalid_value(raw: &str, spec: &Spec) -> JewelRuntimeException {
    let one = format!(
        "Invalid value (Unsupported number format: For input string: \"{}\"): {}",
        raw,
        spec.line()
    );
    let lines: Vec<String> = spec.long.iter().map(|_| one.clone()).collect();
    JewelRuntimeException { message: lines.join("\n") }
}

fn missing_value(spec: &Spec) -> JewelRuntimeException {
    JewelRuntimeException { message: format!("Option must have a value: {}", spec.line()) }
}

fn unexpected(name: &str) -> JewelRuntimeException {
    JewelRuntimeException { message: format!("Unexpected Option: {}", name) }
}

/// `CliFactory.parseArguments(Options.class, args)`.
///
/// Later occurrences win, which is what makes `Launcher.merge` work: the recipe
/// line's arguments are placed first and the real command line second, so an
/// option given on the command line overrides the same option in the recipe.
pub fn parse_arguments<S: AsRef<str>>(args: &[S]) -> Result<Options, JewelRuntimeException> {
    let mut o = Options::default();
    let mut i = 0usize;
    let args: Vec<&str> = args.iter().map(|a| a.as_ref()).collect();

    while i < args.len() {
        let arg = args[i];
        let (name, inline, long) = if let Some(rest) = arg.strip_prefix("--") {
            match rest.split_once('=') {
                Some((n, v)) => (n, Some(v), true),
                None => (rest, None, true),
            }
        } else if arg.len() > 1 && arg.starts_with('-') {
            (&arg[1..], None, false)
        } else {
            // Unparsed: the FILE argument.
            o.file = Some(PathBuf::from(arg));
            i += 1;
            continue;
        };

        let spec = find(name, long).ok_or_else(|| unexpected(name))?;

        let value = if spec.arity == Arity::Value {
            match inline {
                Some(v) => {
                    i += 1;
                    Some(v.to_string())
                }
                None => {
                    // jewelcli treats a following token that looks like another
                    // option as "no value given" rather than swallowing it.
                    let next = args.get(i + 1);
                    match next {
                        Some(v) if !(v.starts_with('-') && v.len() > 1) => {
                            i += 2;
                            Some((*v).to_string())
                        }
                        _ => return Err(missing_value(spec)),
                    }
                }
            }
        } else {
            i += 1;
            None
        };

        let primary = spec.long[0];
        match primary {
            "compact" => o.compact_format = true,
            "data" => o.print_data = true,
            "help" => o.show_help = true,
            "names" => o.print_names = true,
            "verbose" => o.enable_logging = true,
            "output" => o.output = Some(PathBuf::from(value.expect("value option"))),
            "recipe" => o.recipe = Some(PathBuf::from(value.expect("value option"))),
            "filter.category" => o.filter_category = Some(parse_int(&value.expect("value option"), spec)?),
            "filter.discipline" => o.filter_discipline = Some(parse_int(&value.expect("value option"), spec)?),
            "filter.surface" => o.filter_surface = Some(parse_int(&value.expect("value option"), spec)?),
            "filter.value" => o.filter_value = Some(parse_double(&value.expect("value option"), spec)?),
            "filter.parameter" => o.filter_parameter = Some(value.expect("value option")),
            other => unreachable!("grammar and dispatch disagree about {other}"),
        }
    }
    Ok(o)
}
