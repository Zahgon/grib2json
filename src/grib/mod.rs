//! The netCDF-Java GRIB2 decoder surface that grib2json depends on.
//!
//! The Java program does not decode GRIB itself -- it drives
//! `ucar.grib.grib2.Grib2Input` / `Grib2Record` / `Grib2Data` and formats what
//! comes back. This module supplies the same surface in Rust so the writers
//! above it are a direct transcription rather than a redesign.
//!
//! What is decoded: sections 0, 1, 3, 4, 5, 6 and 7 of a GRIB2 message; grid
//! definition templates 3.0-3.3, 3.10, 3.20, 3.30, 3.31, 3.40-3.43, 3.90 and
//! 3.204; product definition templates 4.0-4.15; and data representation
//! template 5.0 (simple packing). See MIGRATION.md for what that leaves out and
//! how an unsupported template is reported.

pub mod data;
pub mod input;
pub mod numbers;
pub mod record;
pub mod tables;

pub use input::Grib2Input;
pub use record::{DataRepresentation, Gds, Grib2Record, IdentificationSection, IndicatorSection, Pds};
