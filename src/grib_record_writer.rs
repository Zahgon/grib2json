//! Port of `net.nullschool.grib2json.GribRecordWriter`.
//!
//! Writes a Grib2 record to a JSON generator. Structure, key order and the
//! template dispatch are unchanged from the Java.

use crate::float_value::FloatValue;
use crate::grib::data::{get_data, Unsupported};
use crate::grib::numbers::{is_bit_set, BIT_5};
use crate::grib::tables::{
    category_name, center_id_name, code_table3_1, code_table3_2, code_table4_0, code_table4_3,
    code_table4_5, parameter_name, parameter_unit,
};
use crate::grib::Grib2Record;
use crate::jtext::DateTime;
use crate::options::Options;
use crate::record_writer::RecordWriter;
use std::io::{self, Write};

/// Return true if the specified command line options do not filter out this record.
///
/// A free function rather than a method: `Grib2Json.write` asks the question
/// *before* it opens the record's Json object, and a `GribRecordWriter` borrows
/// the generator mutably, so no writer can be alive across `writeStartObject`.
pub fn is_selected(record: &Grib2Record, options: &Options) -> bool {
    let ins = &record.is;
    let pds = &record.pds;
    options.filter_discipline.map_or(true, |d| d == ins.discipline)
        && options.filter_category.map_or(true, |c| c == pds.parameter_category)
        && options.filter_surface.map_or(true, |s| s == pds.level_type1)
        // Java unboxes `Double` and compares with `==`, so this is an exact
        // float comparison, not an epsilon one.
        && options.filter_value.map_or(true, |v| v == pds.level_value1)
        && is_selected_parameter(record, options.filter_parameter.as_deref())
}

/// The `filterParameter` half of the selection test.
///
/// The Java is one boolean expression wrapped in `try { } catch
/// (NumberFormatException e) { return false; }`. Note the short-circuit order:
/// a `null` filter accepts everything, `"wind"` is special-cased, and only then
/// is the string parsed as an integer -- so `--fp wind` never reaches
/// `Integer.parseInt` and never throws.
pub fn is_selected_parameter(record: &Grib2Record, filter_parameter: Option<&str>) -> bool {
    let pn = record.pds.parameter_number;
    match filter_parameter {
        None => true,
        Some("wind") if pn == 2 || pn == 3 => true,
        Some(s) => match s.parse::<i32>() {
            Ok(v) => v == pn,
            // The catch clause: an unparsable filter rejects every record.
            Err(_) => false,
        },
    }
}

pub struct GribRecordWriter<'a, W: Write> {
    w: RecordWriter<'a, W>,
    record: &'a Grib2Record,
}

impl<'a, W: Write> GribRecordWriter<'a, W> {
    pub fn new(
        jg: &'a mut crate::json::JsonGenerator<W>,
        record: &'a Grib2Record,
        options: &'a Options,
    ) -> Self {
        GribRecordWriter { w: RecordWriter::new(jg, options), record }
    }

    /// `isSelected()` as the Java instance method.
    pub fn is_selected(&self) -> bool {
        is_selected(self.record, self.w.options)
    }

    /// Write contents of the record's indicator section.
    fn write_indicator(&mut self) -> io::Result<()> {
        let ins = self.record.is.clone();
        let dname = ins.discipline_name();
        self.w.write_named("discipline", ins.discipline, &dname)?;
        self.w.write_int("gribEdition", ins.grib_edition)?;
        self.w.write_long("gribLength", ins.grib_length)
    }

    /// Write contents of the record's identification section.
    fn write_identification(&mut self) -> io::Result<()> {
        let ids = self.record.id.clone();
        self.w.write_named("center", ids.center_id, center_id_name(ids.center_id))?;
        self.w.write_int("subcenter", ids.subcenter_id)?;
        let ref_time = DateTime::from_millis(ids.ref_time).to_iso_string();
        self.w.write_str("refTime", &ref_time)?;
        self.w.write_named("significanceOfRT", ids.significance_of_rt, ids.significance_of_rt_name())?;
        self.w.write_named("productStatus", ids.product_status, ids.product_status_name())?;
        self.w.write_named("productType", ids.product_type, ids.product_type_name())
    }

    /// Write contents of the record's product section.
    fn write_product(&mut self) -> io::Result<()> {
        let pds = self.record.pds.clone();
        let product_def = pds.product_definition_template;
        let discipline = self.record.is.discipline;
        let param_category = pds.parameter_category;
        let param_number = pds.parameter_number;

        self.w.write_named("productDefinitionTemplate", product_def, code_table4_0(product_def))?;
        let cname = category_name(discipline, param_category);
        self.w.write_named("parameterCategory", param_category, &cname)?;
        let pname = parameter_name(discipline, param_category, param_number);
        self.w.write_named("parameterNumber", param_number, &pname)?;
        let punit = parameter_unit(discipline, param_category, param_number);
        self.w.write_str("parameterUnit", &punit)?;
        self.w.write_named("genProcessType", pds.gen_process_type, code_table4_3(pds.gen_process_type))?;
        self.w.write_int("forecastTime", pds.forecast_time)?;
        let s1 = code_table4_5(pds.level_type1);
        self.w.write_named("surface1Type", pds.level_type1, &s1)?;
        self.w.write_double("surface1Value", pds.level_value1)?;
        let s2 = code_table4_5(pds.level_type2);
        self.w.write_named("surface2Type", pds.level_type2, &s2)?;
        self.w.write_double("surface2Value", pds.level_value2)
    }

    fn write_grid_shape(&mut self) -> io::Result<()> {
        // See http://www.nco.ncep.noaa.gov/pmb/docs/grib2/grib2_table3-2.shtml
        let gds = self.record.gds.clone();
        self.w.write_named("shape", gds.shape, code_table3_2(gds.shape))?;
        match gds.shape {
            // Earth assumed spherical with radius specified (in m) by data producer
            1 => self.w.write_float_if_set("earthRadius", gds.earth_radius)?,
            // Earth assumed oblate spheroid with major and minor axes specified (in km) by data producer
            3 => {
                self.w.write_float_if_set("majorAxis", gds.major_axis)?;
                self.w.write_float_if_set("minorAxis", gds.minor_axis)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn write_grid_size(&mut self) -> io::Result<()> {
        let gds = self.record.gds.clone();
        self.w.write_str("gridUnits", gds.grid_units())?;
        self.w.write_int("resolution", gds.resolution)?;
        let winds = if is_bit_set(gds.resolution, BIT_5) { "relative" } else { "true" };
        self.w.write_str("winds", winds)?;
        self.w.write_int("scanMode", gds.scan_mode)?;
        self.w.write_int("nx", gds.nx)?; // Number of points on x-axis or parallel
        self.w.write_int("ny", gds.ny) // Number of points on y-axis or meridian
    }

    fn write_lon_lat_bounds(&mut self) -> io::Result<()> {
        let g = self.record.gds.clone();
        self.w.write_float_if_set("lo1", g.lo1)?; // longitude of first grid point
        self.w.write_float_if_set("la1", g.la1)?; // latitude of first grid point
        self.w.write_float_if_set("lo2", g.lo2)?; // longitude of last grid point
        self.w.write_float_if_set("la2", g.la2)?; // latitude of last grid point
        self.w.write_float_if_set("dx", g.dx)?; // i direction increment
        self.w.write_float_if_set("dy", g.dy) // j direction increment
    }

    fn write_rotation_and_stretch(&mut self) -> io::Result<()> {
        let g = self.record.gds.clone();
        self.w.write_float_if_set("spLon", g.sp_lon)?; // longitude of the southern pole of projection
        self.w.write_float_if_set("spLat", g.sp_lat)?; // latitude of the southern pole of projection
        self.w.write_float_if_set("rotationAngle", g.rotation_angle)?;
        self.w.write_float_if_set("poleLon", g.pole_lon)?; // longitude of the pole stretching
        self.w.write_float_if_set("poleLat", g.pole_lat)?; // latitude of the pole of stretching
        self.w.write_float_if_set("stretchingFactor", g.stretching_factor)
    }

    fn write_angle(&mut self) -> io::Result<()> {
        let g = self.record.gds.clone();
        self.w.write_int_if_set("angle", g.angle)?; // orientation of the grid
        self.w.write_int_if_set("basicAngle", g.basic_angle)?;
        self.w.write_int_if_set("subDivisions", g.sub_divisions)
    }

    fn write_lon_lat_grid(&mut self) -> io::Result<()> {
        self.write_grid_shape()?;
        self.write_grid_size()?;
        self.write_angle()?;
        self.write_lon_lat_bounds()?;
        self.write_rotation_and_stretch()?;
        let np = self.record.gds.np;
        self.w.write_int_if_set("np", np) // number of paralells between a pole and the equator
    }

    fn write_mercator_grid(&mut self) -> io::Result<()> {
        self.write_grid_shape()?;
        self.write_grid_size()?;
        self.write_angle()?;
        self.write_lon_lat_bounds()
    }

    fn write_polar_stereographic_grid(&mut self) -> io::Result<()> {
        self.write_grid_shape()?;
        self.write_grid_size()?;
        self.write_lon_lat_bounds()
    }

    fn write_lambert_conformal_grid(&mut self) -> io::Result<()> {
        self.write_grid_shape()?;
        self.write_grid_size()?;
        self.write_lon_lat_bounds()?;
        self.write_rotation_and_stretch()?;

        let g = self.record.gds.clone();
        self.w.write_float("laD", g.la_d)?;
        self.w.write_float("loV", g.lo_v)?;
        self.w.write_int("projectionFlag", g.projection_flag)?;
        // first latitude from the pole at which the secant cone cuts the sphere
        self.w.write_float("latin1", g.latin1)?;
        // second latitude from the pole at which the secant cone cuts the sphere
        self.w.write_float("latin2", g.latin2)
    }

    fn write_space_or_orthographic_grid(&mut self) -> io::Result<()> {
        self.write_grid_shape()?;
        self.write_grid_size()?;
        self.write_angle()?;
        self.write_lon_lat_bounds()?;

        let g = self.record.gds.clone();
        self.w.write_float("lop", g.lop)?; // longitude of sub-satellite point
        self.w.write_float("lap", g.lap)?; // latitude of sub-satellite point
        self.w.write_float("xp", g.xp)?; // x-coordinate of sub-satellite point
        self.w.write_float("yp", g.yp)?; // y-coordinate of sub-satellite point
        self.w.write_float("nr", g.nr)?; // altitude of the camera from the Earth's center
        self.w.write_float("xo", g.xo)?; // x-coordinate of origin of sector image
        self.w.write_float("yo", g.yo) // y-coordinate of origin of sector image
    }

    fn write_curvilinear_grid(&mut self) -> io::Result<()> {
        self.write_grid_shape()?;
        self.write_grid_size()
    }

    /// Write contents of the record's grid definition section.
    /// See http://www.nco.ncep.noaa.gov/pmb/docs/grib2/grib2_table3-1.shtml
    fn write_grid_definition(&mut self) -> io::Result<()> {
        let grid_template = self.record.gds.gdtn;
        let number_points = self.record.gds.number_points;

        let gname = code_table3_1(grid_template);
        self.w.write_named("gridDefinitionTemplate", grid_template, &gname)?;
        self.w.write_int("numberPoints", number_points)?;

        match grid_template {
            // Templates 3.0 - 3.3
            0 | 1 | 2 | 3 => self.write_lon_lat_grid(),
            // Template 3.10
            10 => self.write_mercator_grid(),
            // Template 3.20
            20 => self.write_polar_stereographic_grid(),
            // Template 3.30
            30 => self.write_lambert_conformal_grid(),
            // Templates 3.40 - 3.43
            40 | 41 | 42 | 43 => self.write_lon_lat_grid(),
            // Template 3.90
            90 => self.write_space_or_orthographic_grid(),
            // Template 3.204
            204 => self.write_curvilinear_grid(),
            _ => Ok(()),
        }
    }

    /// Write the record's header as a Json object: `"header": { ... }`
    pub fn write_header(&mut self) -> io::Result<()> {
        self.w.jg.write_start_object_named("header")?;
        self.write_indicator()?;
        self.write_identification()?;
        self.write_product()?;
        self.write_grid_definition()?;
        self.w.jg.write_end()
    }

    /// Write the record's data as a Json array: `"data": [ ... ]`
    ///
    /// `Grib2Data.getData` returning `null` is the Java signal that the record
    /// could not be unpacked; the array is then omitted entirely. An
    /// unsupported packing template takes that same path here.
    pub fn write_data(&mut self) -> io::Result<Option<Unsupported>> {
        match get_data(self.record) {
            Err(reason) => Ok(Some(reason)),
            Ok(data) => {
                self.w.jg.write_start_array_named("data")?;
                for value in data {
                    self.w.jg.write_float_value_item(FloatValue::new(value))?;
                }
                self.w.jg.write_end()?;
                Ok(None)
            }
        }
    }
}
