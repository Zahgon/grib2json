//! Port of `net.nullschool.grib2json.OscarRecordWriter`.
//!
//! Writes OSCAR (Ocean Surface Current Analyses Real-time) data to a JSON
//! generator.
//!
//! Lots of magic values in here, as we're mapping OSCAR data in NetCDF format to
//! JSON using GRIB header constants. The input is assumed to be in the range
//! [20E:80N, 380E:80S], with 1/3º resolution. To reduce download size, the
//! precision of the data is reduced to a resolution of 2cm/s.
//!
//! This is not a generic processing of NetCDF records.
//!
//! For more information on OSCAR, see http://www.esr.org/oscar_index.html.

use crate::float_value::FloatValue;
use crate::grib::tables::{
    category_name, code_table3_2, code_table4_5, parameter_name, parameter_table_discipline_name,
    parameter_unit,
};
use crate::jtext::DateTime;
use crate::netcdf::{NetcdfError, NetcdfFile};
use crate::options::Options;
use crate::record_writer::RecordWriter;
use std::io::{self, Write};

const OCEAN_PRODUCTS: i32 = 10;
/// Number of points on x-axis or parallel
const NX: i32 = 1080;
/// Number of points on y-axis or meridian
const NY: i32 = 481;
const RANGE: &str = "0,0,0:480,0:1079";

/// The failure `writeData` can raise, standing in for the Java
/// `catch (InvalidRangeException e) { throw new RuntimeException(e); }`.
#[derive(Debug)]
pub enum OscarError {
    Io(io::Error),
    Netcdf(NetcdfError),
    /// `variableToParam`'s `IllegalArgumentException`.
    UnknownVariable(String),
}

impl std::fmt::Display for OscarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OscarError::Io(e) => write!(f, "{}", e),
            OscarError::Netcdf(e) => write!(f, "{}", e),
            OscarError::UnknownVariable(v) => write!(f, "unknown variable: {}", v),
        }
    }
}

impl From<io::Error> for OscarError {
    fn from(e: io::Error) -> Self {
        OscarError::Io(e)
    }
}

impl From<NetcdfError> for OscarError {
    fn from(e: NetcdfError) -> Self {
        OscarError::Netcdf(e)
    }
}

pub struct OscarRecordWriter<'a, W: Write> {
    w: RecordWriter<'a, W>,
    file: &'a NetcdfFile,
    var: String,
    date: DateTime,
    depth: f64,
}

impl<'a, W: Write> OscarRecordWriter<'a, W> {
    pub fn new(
        jg: &'a mut crate::json::JsonGenerator<W>,
        file: &'a NetcdfFile,
        var: &str,
        date: DateTime,
        depth: f64,
        options: &'a Options,
    ) -> Self {
        OscarRecordWriter {
            w: RecordWriter::new(jg, options),
            file,
            var: var.to_string(),
            date,
            depth,
        }
    }

    fn write_indicator(&mut self) -> io::Result<()> {
        let name = parameter_table_discipline_name(OCEAN_PRODUCTS);
        self.w.write_named("discipline", OCEAN_PRODUCTS, &name)
    }

    fn write_identification(&mut self) -> io::Result<()> {
        self.w.write_named("center", -3, "Earth & Space Research")?;
        let ref_time = self.date.to_iso_string();
        self.w.write_str("refTime", &ref_time)?;
        self.w.write_named("significanceOfRT", 0, "Analysis")
    }

    /// `variableToParam(Variable)`.
    fn variable_to_param(name: &str) -> Result<i32, OscarError> {
        match name {
            "u" => Ok(2), // U component of current
            "v" => Ok(3), // V component of current
            other => Err(OscarError::UnknownVariable(other.to_string())),
        }
    }

    fn write_product(&mut self) -> Result<(), OscarError> {
        let param_category = 1; // Currents
        let param_number = Self::variable_to_param(&self.var)?;
        let surface_type = 160; // Depth below sea level

        let cname = category_name(OCEAN_PRODUCTS, param_category);
        self.w.write_named("parameterCategory", 1, &cname)?;
        let pname = parameter_name(OCEAN_PRODUCTS, param_category, param_number);
        self.w.write_named("parameterNumber", param_number, &pname)?;
        let punit = parameter_unit(OCEAN_PRODUCTS, param_category, param_number);
        self.w.write_str("parameterUnit", &punit)?;
        self.w.write_int("forecastTime", 0)?;
        let sname = code_table4_5(surface_type);
        self.w.write_named("surface1Type", surface_type, &sname)?;
        let depth = self.depth;
        self.w.write_double("surface1Value", depth)?;
        Ok(())
    }

    fn write_grid_shape(&mut self) -> io::Result<()> {
        self.w.write_named("shape", 0, code_table3_2(0))
    }

    fn write_grid_size(&mut self) -> io::Result<()> {
        self.w.write_int("scanMode", 0)?;
        self.w.write_int("nx", NX)?; // Number of points on x-axis or parallel
        self.w.write_int("ny", NY) // Number of points on y-axis or meridian
    }

    fn write_lon_lat_bounds(&mut self) -> io::Result<()> {
        self.w.write_int("lo1", 20)?; // longitude of first grid point
        self.w.write_int("la1", 80)?; // latitude of first grid point
        // The Java writes `(20 + 359) + 2/3d`, an int sum widened by a double
        // division -- 379.6666666666667, not 379.66666666666663.
        self.w.write_double("lo2", (20 + 359) as f64 + 2.0 / 3.0)?; // longitude of last grid point
        self.w.write_int("la2", -80)?; // latitude of last grid point
        self.w.write_double("dx", 1.0 / 3.0)?; // i direction increment
        self.w.write_double("dy", 1.0 / 3.0) // j direction increment
    }

    fn write_grid_definition(&mut self) -> io::Result<()> {
        self.w.write_int("numberPoints", NX * NY)?;
        self.write_grid_shape()?;
        self.write_grid_size()?;
        self.write_lon_lat_bounds()
    }

    /// Write the record's header as a Json object: `"header": { ... }`
    fn write_header(&mut self) -> Result<(), OscarError> {
        self.w.jg.write_start_object_named("header")?;
        self.write_indicator()?;
        self.write_identification()?;
        self.write_product()?;
        self.write_grid_definition()?;
        self.w.jg.write_end()?;
        Ok(())
    }

    /// Round to nearest fraction of 1/denominator.
    ///
    /// `Math.round(float)` returns the nearest `int`, breaking ties by rounding
    /// *up* (toward positive infinity), which differs from Rust's `f32::round`
    /// for negative halves: Java rounds -2.5 to -2, Rust rounds it to -3.
    fn round(value: f32, denominator: f32) -> f32 {
        let scaled = value * denominator;
        let rounded = (scaled + 0.5).floor();
        rounded / denominator
    }

    /// Write the record's data as a Json array: `"data": [ ... ]`
    fn write_data(&mut self) -> Result<(), OscarError> {
        if !self.w.options.print_data {
            return Ok(());
        }
        self.w.jg.write_start_array_named("data")?;
        let data = self.file.read_section(&self.var, RANGE)?;
        for value in data {
            if value.is_nan() {
                self.w.jg.write_null_item()?;
            } else {
                self.w.jg.write_float_value_item(FloatValue::new(Self::round(value, 50.0)))?;
            }
        }
        self.w.jg.write_end()?;
        Ok(())
    }

    /// Write the record as a Json object: `{ "header": { ... }, "data": [ ... ] }`
    pub fn write_record(&mut self) -> Result<(), OscarError> {
        self.w.jg.write_start_object()?;
        self.write_header()?;
        self.write_data()?;
        self.w.jg.write_end()?;
        Ok(())
    }
}
