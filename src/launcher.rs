//! Port of `net.nullschool.grib2json.Launcher`.
//!
//! Execution shim for the grib2json utility. Parses command line options and
//! invokes the [`Grib2Json`](crate::grib2json::Grib2Json) converter.
//!
//! `main` is a thin wrapper over [`run`], which takes its streams as arguments
//! so the exit status, stdout and stderr of every branch can be asserted in a
//! test without spawning a process.

use crate::grib2json::{Grib2Json, Grib2JsonError};
use crate::options::{help_message, parse_arguments, JewelRuntimeException, Options};
use std::io::Write;
use std::path::Path;

/// Exit statuses, matching `System.exit` in the Java.
pub const EXIT_OK: i32 = 0;
/// A usage error, a bad argument, or an `IllegalArgumentException`.
pub const EXIT_USAGE: i32 = 1;
/// Anything else -- the `catch (Throwable t)` arm.
pub const EXIT_ERROR: i32 = 2;

/// `printUsage()`.
fn print_usage(out: &mut dyn Write) {
    let _ = writeln!(out, "{}", help_message());
}

/// `merge(T[] a, T[] b)` -- `a` first, then `b`.
fn merge(a: Vec<String>, b: &[String]) -> Vec<String> {
    let mut result = a;
    result.extend_from_slice(b);
    result
}

/// `splitArgs(String line)` -- split on runs of whitespace, dropping empties.
fn split_args(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

/// `readRecipeFile(String[] mainArgs, File recipe)`.
///
/// Each line of the recipe becomes its own option group. The recipe's arguments
/// go first and the real command line second, so a command-line option
/// overrides the same option in the recipe.
fn read_recipe_file(
    main_args: &[String],
    recipe: &Path,
    log: &mut dyn Write,
    verbose: bool,
) -> Result<Vec<Options>, LauncherError> {
    let text = std::fs::read_to_string(recipe).map_err(LauncherError::Io)?;
    let mut groups = Vec::new();
    for line in text.lines() {
        let args = merge(split_args(line), main_args);
        if verbose {
            // `log.info(Arrays.toString(args))`
            let _ = writeln!(log, "[{}]", args.join(", "));
        }
        groups.push(parse_arguments(&args).map_err(LauncherError::Jewel)?);
    }
    Ok(groups)
}

/// The three failure shapes `main` catches, in the order the Java catches them.
enum LauncherError {
    Jewel(JewelRuntimeException),
    IllegalArgument(String),
    Io(std::io::Error),
}

impl From<Grib2JsonError> for LauncherError {
    fn from(e: Grib2JsonError) -> Self {
        match e {
            Grib2JsonError::IllegalArgument(m) => LauncherError::IllegalArgument(m),
            Grib2JsonError::Io(e) => LauncherError::Io(e),
            // A NetCDF or OSCAR failure is an ordinary Throwable in the Java, so
            // it lands in the last catch arm and exits 2.
            Grib2JsonError::Oscar(m) => {
                LauncherError::Io(std::io::Error::other(m))
            }
        }
    }
}

/// `main(String[] args)`, with its streams injected.
///
/// Returns the process exit status. The dispatch below is the Java `try` block
/// and its three `catch` clauses, in the same order and with the same effects:
///
/// * `JewelRuntimeException` -> usage on stdout, a blank line, message on stderr, exit 1
/// * `IllegalArgumentException` -> message on stderr, exit 1
/// * anything else -> the error on stderr, exit 2
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match try_run(args, out, err) {
        Ok(status) => status,
        Err(LauncherError::Jewel(e)) => {
            print_usage(out);
            // `System.out.println()` -- the blank line between usage and message.
            let _ = writeln!(out);
            let _ = writeln!(err, "{}", e.message);
            EXIT_USAGE
        }
        Err(LauncherError::IllegalArgument(message)) => {
            let _ = writeln!(err, "{}", message);
            EXIT_USAGE
        }
        Err(LauncherError::Io(e)) => {
            let _ = writeln!(err, "{}", e);
            EXIT_ERROR
        }
    }
}

fn try_run(
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32, LauncherError> {
    let options = parse_arguments(args).map_err(LauncherError::Jewel)?;

    if options.show_help || options.file.is_none() {
        print_usage(out);
        return Ok(if options.show_help { EXIT_OK } else { EXIT_USAGE });
    }

    // `lc.stop()` when logging is off. There is no logging framework here; the
    // flag is threaded through to the two places that would have logged.
    let verbose = options.enable_logging;

    let option_groups = match &options.recipe {
        Some(recipe) => read_recipe_file(args, recipe, err, verbose)?,
        None => vec![options.clone()],
    };

    let file = options.file.clone().expect("checked above");
    Grib2Json::new(&file, option_groups)?.write(out)?;
    Ok(EXIT_OK)
}
