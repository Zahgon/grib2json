//! GRIB2 message sections, in the shape grib2json consumes them.
//!
//! The Java program reaches the decoder through five netCDF-Java types --
//! `Grib2Record`, `Grib2IndicatorSection`, `Grib2IdentificationSection`,
//! `Grib2Pds` and `Grib2GDSVariables`. Those are reproduced here as plain
//! structs with the same accessor names, so `GribRecordWriter` reads exactly
//! the way it reads in Java.
//!
//! Every integer field is initialised to [`UNDEFINED`], which is what makes
//! `writeIfSet` behave identically: a template that does not carry `spLon`
//! leaves it undefined and the key never reaches the JSON.

use super::numbers::{code, float, ibm_float, int, uint, ulong, UNDEFINED, UNDEFINED_D};
use super::tables;

/// Section 0. `ucar.grib.grib2.Grib2IndicatorSection`.
#[derive(Debug, Clone)]
pub struct IndicatorSection {
    pub discipline: i32,
    pub grib_edition: i32,
    pub grib_length: i64,
}

impl IndicatorSection {
    pub fn discipline_name(&self) -> String {
        tables::discipline_name(self.discipline)
    }
}

/// Section 1. `ucar.grib.grib2.Grib2IdentificationSection`.
#[derive(Debug, Clone)]
pub struct IdentificationSection {
    pub center_id: i32,
    pub subcenter_id: i32,
    pub significance_of_rt: i32,
    /// Milliseconds since the Unix epoch, the units `getRefTime()` returns.
    pub ref_time: i64,
    pub product_status: i32,
    pub product_type: i32,
}

impl IdentificationSection {
    pub fn significance_of_rt_name(&self) -> &'static str {
        tables::significance_of_rt_name(self.significance_of_rt)
    }
    pub fn product_status_name(&self) -> &'static str {
        tables::product_status_name(self.product_status)
    }
    pub fn product_type_name(&self) -> &'static str {
        tables::product_type_name(self.product_type)
    }
}

/// Section 4 variables. `ucar.grib.grib2.Grib2Pds`.
#[derive(Debug, Clone)]
pub struct Pds {
    pub product_definition_template: i32,
    pub parameter_category: i32,
    pub parameter_number: i32,
    pub gen_process_type: i32,
    pub forecast_time: i32,
    pub level_type1: i32,
    pub level_value1: f64,
    pub level_type2: i32,
    pub level_value2: f64,
}

/// Section 3 variables. `ucar.grib.grib2.Grib2GDSVariables`.
///
/// One flat struct rather than a template hierarchy, because that is how
/// netCDF-Java presents it to a caller and how `GribRecordWriter` switches on it.
#[derive(Debug, Clone)]
pub struct Gds {
    pub gdtn: i32,
    pub number_points: i32,
    pub shape: i32,
    pub earth_radius: f32,
    pub major_axis: f32,
    pub minor_axis: f32,
    pub nx: i32,
    pub ny: i32,
    pub resolution: i32,
    pub scan_mode: i32,
    pub lo1: f32,
    pub la1: f32,
    pub lo2: f32,
    pub la2: f32,
    pub dx: f32,
    pub dy: f32,
    pub sp_lon: f32,
    pub sp_lat: f32,
    pub rotation_angle: f32,
    pub pole_lon: f32,
    pub pole_lat: f32,
    pub stretching_factor: f32,
    pub angle: i32,
    pub basic_angle: i32,
    pub sub_divisions: i32,
    pub np: i32,
    pub la_d: f32,
    pub lo_v: f32,
    pub projection_flag: i32,
    pub latin1: f32,
    pub latin2: f32,
    pub lop: f32,
    pub lap: f32,
    pub xp: f32,
    pub yp: f32,
    pub nr: f32,
    pub xo: f32,
    pub yo: f32,
}

impl Default for Gds {
    fn default() -> Self {
        let u = UNDEFINED;
        let uf = UNDEFINED as f32;
        Gds {
            gdtn: u,
            number_points: u,
            shape: u,
            earth_radius: uf,
            major_axis: uf,
            minor_axis: uf,
            nx: u,
            ny: u,
            resolution: u,
            scan_mode: u,
            lo1: uf,
            la1: uf,
            lo2: uf,
            la2: uf,
            dx: uf,
            dy: uf,
            sp_lon: uf,
            sp_lat: uf,
            rotation_angle: uf,
            pole_lon: uf,
            pole_lat: uf,
            stretching_factor: uf,
            angle: u,
            basic_angle: u,
            sub_divisions: u,
            np: u,
            la_d: uf,
            lo_v: uf,
            projection_flag: u,
            latin1: uf,
            latin2: uf,
            lop: uf,
            lap: uf,
            xp: uf,
            yp: uf,
            nr: uf,
            xo: uf,
            yo: uf,
        }
    }
}

impl Gds {
    /// `Grib2GDSVariables.getGridUnits()`.
    ///
    /// netCDF-Java reports the unit the grid increments are expressed in:
    /// degrees for the geographic templates, metres for the projected ones,
    /// and the empty string when the template has no increments at all.
    pub fn grid_units(&self) -> &'static str {
        match self.gdtn {
            0..=3 | 40..=43 | 204 => "degrees",
            10 | 20 | 30 | 31 => "m",
            // Space view carries no linear increment at all: its Dx/Dy are
            // counts of grid lengths across the apparent Earth diameter, so
            // netCDF-Java reports no unit.
            _ => "",
        }
    }
}

/// Data representation (section 5) plus the packed payload (section 7).
#[derive(Debug, Clone)]
pub struct DataRepresentation {
    pub template: i32,
    pub number_of_values: i32,
    pub reference_value: f32,
    pub binary_scale_factor: i32,
    pub decimal_scale_factor: i32,
    pub number_of_bits: i32,
}

/// One decoded GRIB2 message. `ucar.grib.grib2.Grib2Record`.
#[derive(Debug, Clone)]
pub struct Grib2Record {
    pub is: IndicatorSection,
    pub id: IdentificationSection,
    pub pds: Pds,
    pub gds: Gds,
    pub drs: Option<DataRepresentation>,
    /// Section 6 bitmap, when one is present, as a bit-per-point run.
    pub bitmap: Option<Vec<u8>>,
    /// Section 7 payload.
    pub data_bytes: Vec<u8>,
}

/// Scale a GRIB "scale factor / scaled value" pair into a real number.
///
/// A missing scale factor or value yields the section's undefined sentinel,
/// which is what puts `-9.999E-252` into the header for an absent second surface.
fn scaled(factor: i32, value: i32) -> f64 {
    if factor == UNDEFINED || value == UNDEFINED {
        return UNDEFINED_D;
    }
    if factor == 0 {
        value as f64
    } else {
        value as f64 / 10f64.powi(factor)
    }
}

/// Scaled optional grid lengths use the geometry sentinel, not the double
/// surface sentinel: converting the latter to f32 would underflow to -0.0.
fn scaled_geometry(factor: i32, value: i32, units: f64) -> f32 {
    if factor == UNDEFINED || value == UNDEFINED {
        UNDEFINED as f32
    } else {
        (scaled(factor, value) * units) as f32
    }
}

/// A field netCDF-Java reports as-is, widened to float.
fn raw_float(raw: i32) -> f32 {
    if raw == UNDEFINED {
        UNDEFINED as f32
    } else {
        raw as f32
    }
}

/// Metres, from the millimetre integers the projected templates store.
fn millimetres(raw: i32) -> f32 {
    if raw == UNDEFINED {
        UNDEFINED as f32
    } else {
        (raw as f64 / 1000.0) as f32
    }
}

/// Degrees, from the 1e-6-degree integers the geographic templates store.
fn degrees(raw: i32) -> f32 {
    if raw == UNDEFINED {
        UNDEFINED as f32
    } else {
        (raw as f64 / 1_000_000.0) as f32
    }
}

/// Parse section 1 out of `s`, whose first octet is the section length.
pub fn parse_identification(s: &[u8]) -> IdentificationSection {
    let year = uint(s, 12, 2);
    let month = uint(s, 14, 1);
    let day = uint(s, 15, 1);
    let hour = uint(s, 16, 1);
    let minute = uint(s, 17, 1);
    let second = uint(s, 18, 1);
    let dt = crate::jtext::DateTime::from_utc(year as i64, month as i64, day as i64, hour as i64, minute as i64);
    IdentificationSection {
        center_id: uint(s, 5, 2),
        subcenter_id: uint(s, 7, 2),
        significance_of_rt: code(s, 11),
        ref_time: dt.millis + second.max(0) as i64 * 1000,
        product_status: code(s, 19),
        product_type: code(s, 20),
    }
}

/// Parse section 4.
///
/// Only the octets `GribRecordWriter` reads are interpreted. Templates 4.0
/// through 4.15 share the same prefix up to the second fixed surface, which is
/// the whole of what is read here; templates outside that family report the
/// prefix fields as undefined rather than misreading unrelated octets.
pub fn parse_pds(s: &[u8]) -> Pds {
    let template = uint(s, 7, 2);
    // Octet offsets below are 0-based from the start of the section.
    let known_prefix = matches!(template, 0..=15);
    if !known_prefix {
        return Pds {
            product_definition_template: template,
            parameter_category: UNDEFINED,
            parameter_number: UNDEFINED,
            gen_process_type: UNDEFINED,
            forecast_time: UNDEFINED,
            level_type1: UNDEFINED,
            level_value1: UNDEFINED_D,
            level_type2: UNDEFINED,
            level_value2: UNDEFINED_D,
        };
    }
    Pds {
        product_definition_template: template,
        parameter_category: code(s, 9),
        parameter_number: code(s, 10),
        gen_process_type: code(s, 11),
        forecast_time: int(s, 18, 4),
        level_type1: code(s, 22),
        level_value1: scaled(int(s, 23, 1), int(s, 24, 4)),
        level_type2: code(s, 28),
        level_value2: scaled(int(s, 29, 1), int(s, 30, 4)),
    }
}

/// Parse section 3, dispatching on the grid definition template number.
pub fn parse_gds(s: &[u8]) -> Gds {
    let mut g = Gds { gdtn: uint(s, 12, 2), number_points: int(s, 6, 4), ..Default::default() };

    // The earth-shape block is shared by every template that has a body at all.
    if s.len() > 30 {
        g.shape = code(s, 14);
        g.earth_radius = scaled_geometry(int(s, 15, 1), int(s, 16, 4), 1.0);
        // Octets 21-30 hold the axes in km; netCDF-Java reports metres.
        g.major_axis = scaled_geometry(int(s, 20, 1), int(s, 21, 4), 1000.0);
        g.minor_axis = scaled_geometry(int(s, 25, 1), int(s, 26, 4), 1000.0);
    }

    match g.gdtn {
        // Latitude/longitude family, and the Gaussian family which shares its layout.
        0..=3 | 40..=43 => {
            g.nx = int(s, 30, 4);
            g.ny = int(s, 34, 4);
            g.basic_angle = int(s, 38, 4);
            g.sub_divisions = int(s, 42, 4);
            g.la1 = degrees(int(s, 46, 4));
            g.lo1 = degrees(int(s, 50, 4));
            g.resolution = code(s, 54);
            g.la2 = degrees(int(s, 55, 4));
            g.lo2 = degrees(int(s, 59, 4));
            g.dx = degrees(int(s, 63, 4));
            if matches!(g.gdtn, 40..=43) {
                // The Gaussian templates carry N, the number of parallels between
                // a pole and the equator, where the lat/lon templates carry Dj.
                g.np = int(s, 67, 4);
            } else {
                g.dy = degrees(int(s, 67, 4));
            }
            g.scan_mode = code(s, 71);
            if matches!(g.gdtn, 1 | 3 | 41 | 43) {
                g.sp_lat = degrees(int(s, 72, 4));
                g.sp_lon = degrees(int(s, 76, 4));
                // An IBM-format float, not IEEE -- see numbers::ibm_float.
                g.rotation_angle = ibm_float(s, 80);
            }
            if matches!(g.gdtn, 2 | 3 | 42 | 43) {
                let base = if matches!(g.gdtn, 3 | 43) { 84 } else { 72 };
                g.pole_lat = degrees(int(s, base, 4));
                g.pole_lon = degrees(int(s, base + 4, 4));
                g.stretching_factor = ibm_float(s, base + 8);
            }
        }
        // Mercator.
        10 => {
            g.nx = int(s, 30, 4);
            g.ny = int(s, 34, 4);
            g.la1 = degrees(int(s, 38, 4));
            g.lo1 = degrees(int(s, 42, 4));
            g.resolution = code(s, 46);
            g.latin1 = degrees(int(s, 47, 4));
            g.la2 = degrees(int(s, 51, 4));
            g.lo2 = degrees(int(s, 55, 4));
            g.scan_mode = code(s, 59);
            g.angle = int(s, 60, 4);
            g.dx = millimetres(int(s, 64, 4));
            g.dy = millimetres(int(s, 68, 4));
        }
        // Polar stereographic.
        20 => {
            g.nx = int(s, 30, 4);
            g.ny = int(s, 34, 4);
            g.la1 = degrees(int(s, 38, 4));
            g.lo1 = degrees(int(s, 42, 4));
            g.resolution = code(s, 46);
            g.la_d = degrees(int(s, 47, 4));
            g.lo_v = degrees(int(s, 51, 4));
            g.dx = millimetres(int(s, 55, 4));
            g.dy = millimetres(int(s, 59, 4));
            g.projection_flag = code(s, 63);
            g.scan_mode = code(s, 64);
        }
        // Lambert conformal, and Albers which shares its layout.
        30 | 31 => {
            g.nx = int(s, 30, 4);
            g.ny = int(s, 34, 4);
            g.la1 = degrees(int(s, 38, 4));
            g.lo1 = degrees(int(s, 42, 4));
            g.resolution = code(s, 46);
            g.la_d = degrees(int(s, 47, 4));
            g.lo_v = degrees(int(s, 51, 4));
            g.dx = millimetres(int(s, 55, 4));
            g.dy = millimetres(int(s, 59, 4));
            g.projection_flag = code(s, 63);
            g.scan_mode = code(s, 64);
            g.latin1 = degrees(int(s, 65, 4));
            g.latin2 = degrees(int(s, 69, 4));
            g.sp_lat = degrees(int(s, 73, 4));
            g.sp_lon = degrees(int(s, 77, 4));
        }
        // Space view perspective or orthographic.
        90 => {
            g.nx = int(s, 30, 4);
            g.ny = int(s, 34, 4);
            // netCDF-Java hands the space-view fields back unscaled, except
            // Xp/Yp which it divides by 1000. Reproduced rather than corrected:
            // `lop` really does print as 1.407E8 for a sub-satellite longitude
            // of 140.7 degrees.
            g.lap = raw_float(int(s, 38, 4));
            g.lop = raw_float(int(s, 42, 4));
            g.resolution = code(s, 46);
            g.dx = raw_float(int(s, 47, 4));
            g.dy = raw_float(int(s, 51, 4));
            g.xp = (int(s, 55, 4) as f64 / 1000.0) as f32;
            g.yp = (int(s, 59, 4) as f64 / 1000.0) as f32;
            g.scan_mode = code(s, 63);
            g.angle = int(s, 64, 4);
            g.nr = raw_float(int(s, 68, 4));
            g.xo = raw_float(int(s, 72, 4));
            g.yo = raw_float(int(s, 76, 4));
        }
        // Curvilinear orthogonal grids: shape and size only.
        204 => {
            g.nx = int(s, 30, 4);
            g.ny = int(s, 34, 4);
            g.resolution = code(s, 54);
            g.scan_mode = code(s, 71);
        }
        _ => {}
    }
    g
}

/// Parse section 5.
pub fn parse_drs(s: &[u8]) -> DataRepresentation {
    DataRepresentation {
        number_of_values: int(s, 5, 4),
        template: uint(s, 9, 2),
        reference_value: float(s, 11),
        binary_scale_factor: int(s, 15, 2),
        decimal_scale_factor: int(s, 17, 2),
        number_of_bits: code(s, 19),
    }
}

/// Total message length from section 0.
pub fn message_length(buf: &[u8], off: usize) -> i64 {
    ulong(buf, off + 8)
}
