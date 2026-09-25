//! grib2json -- a command line utility that decodes GRIB2 files as JSON.
//!
//! Rust port of <https://github.com/cambecc/grib2json> at commit
//! `7fb455d745854e336a4462808aca8d791232ef84`, MIT licensed, © 2013 Cameron
//! Beccario.
//!
//! # Module map
//!
//! Each Java class has exactly one Rust module:
//!
//! | Java                        | Rust                        |
//! |-----------------------------|-----------------------------|
//! | `Launcher`                  | [`launcher`]                |
//! | `Options`                   | [`options`]                 |
//! | `Grib2Json`                 | [`grib2json`]               |
//! | `AbstractRecordWriter`      | [`record_writer`]           |
//! | `GribRecordWriter`          | [`grib_record_writer`]      |
//! | `OscarRecordWriter`         | [`oscar_record_writer`]     |
//! | `FloatValue`                | [`float_value`]             |
//!
//! The library dependencies the Java build pulls in are replaced by modules
//! rather than crates, because each of them is observable in the output:
//! [`json`] for `javax.json`, [`jtext`] for `Float.toString` / `Double.toString`
//! / joda-time, [`grib`] for `edu.ucar:grib`, and [`netcdf`] for the slice of
//! `ucar.nc2` the OSCAR path uses.

pub mod float_value;
pub mod grib;
pub mod grib2json;
pub mod grib_record_writer;
pub mod json;
pub mod jtext;
pub mod launcher;
pub mod netcdf;
pub mod options;
pub mod oscar_record_writer;
pub mod record_writer;
