//! Shared helpers for the integration tests.
//!
//! `launcher::run` takes its streams as arguments precisely so the tests can
//! capture stdout and stderr without spawning a process, which keeps every
//! assertion below in-process and makes them work under coverage instrumentation.

//!
//! Each integration test is its own crate, so a helper only some of them use
//! reads as dead code in the others -- `tables_test` needs `fixture` but never
//! drives the launcher.
#![allow(dead_code)]

use std::path::PathBuf;

/// Absolute path to a file under `tests/fixtures/`.
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// Drive the launcher and return `(exit status, stdout, stderr)`.
pub fn run(args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();
    let status = grib2json::launcher::run(&args, &mut out, &mut err);
    (
        status,
        String::from_utf8(out).expect("stdout is UTF-8"),
        String::from_utf8(err).expect("stderr is UTF-8"),
    )
}
