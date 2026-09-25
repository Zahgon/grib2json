//! Port of `net.nullschool.grib2json.Grib2Json`.
//!
//! Converts a GRIB2 file to Json. GRIB2 decoding is performed by
//! [`crate::grib`], the Rust counterpart of the netCDF-Java GRIB decoder.

use crate::grib::Grib2Input;
use crate::grib_record_writer::{is_selected, GribRecordWriter};
use crate::json::JsonGenerator;
use crate::jtext::DateTime;
use crate::netcdf::NetcdfFile;
use crate::options::Options;
use crate::oscar_record_writer::{OscarError, OscarRecordWriter};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Errors `write()` can surface, matching the three Java exit paths:
/// `IllegalArgumentException` (exit 1) and anything else (exit 2).
#[derive(Debug)]
pub enum Grib2JsonError {
    /// `new IllegalArgumentException("Cannot find input file: " + file)`.
    IllegalArgument(String),
    Io(io::Error),
    Oscar(String),
}

impl std::fmt::Display for Grib2JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grib2JsonError::IllegalArgument(m) => f.write_str(m),
            Grib2JsonError::Io(e) => write!(f, "{}", e),
            Grib2JsonError::Oscar(m) => f.write_str(m),
        }
    }
}

impl From<io::Error> for Grib2JsonError {
    fn from(e: io::Error) -> Self {
        Grib2JsonError::Io(e)
    }
}

impl From<OscarError> for Grib2JsonError {
    fn from(e: OscarError) -> Self {
        Grib2JsonError::Oscar(e.to_string())
    }
}

/// Where a generator's bytes go: a file named by `--output`, or the process
/// stdout the caller supplies.
///
/// The Java writes straight to `System.out` when `--output` is absent. Here the
/// stream is a parameter instead of a global, which changes nothing about what
/// a user sees and lets the tests assert on a document without spawning a
/// process -- and lets them run under coverage instrumentation.
enum Sink<'a> {
    File(BufWriter<File>),
    Stream(&'a mut dyn Write),
}

impl Write for Sink<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Sink::File(w) => w.write(buf),
            Sink::Stream(w) => w.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Sink::File(w) => w.flush(),
            Sink::Stream(w) => w.flush(),
        }
    }
}

pub struct Grib2Json {
    file: PathBuf,
    option_groups: Vec<Options>,
}

impl Grib2Json {
    /// `new Grib2Json(File, List<Options>)`.
    ///
    /// The existence check is in the constructor in Java too, and it throws
    /// `IllegalArgumentException`, which `Launcher` turns into a bare stderr
    /// line and exit status 1.
    pub fn new(file: &Path, option_groups: Vec<Options>) -> Result<Self, Grib2JsonError> {
        if !file.exists() {
            return Err(Grib2JsonError::IllegalArgument(format!(
                "Cannot find input file: {}",
                file.display()
            )));
        }
        Ok(Grib2Json { file: file.to_path_buf(), option_groups })
    }

    fn new_json_generator<'a>(
        options: &Options,
        stdout: &'a mut dyn Write,
    ) -> io::Result<JsonGenerator<Sink<'a>>> {
        let sink = match &options.output {
            Some(path) => Sink::File(BufWriter::new(File::create(path)?)),
            None => Sink::Stream(stdout),
        };
        Ok(JsonGenerator::new(sink, !options.compact_format))
    }

    /// The GRIB branch of `write(RandomAccessFile, Grib2Input, Options)`.
    fn write_grib(
        input: &Grib2Input,
        options: &Options,
        stdout: &mut dyn Write,
    ) -> io::Result<()> {
        let mut jg = Self::new_json_generator(options, stdout)?;
        jg.write_start_array()?;

        for record in input.records() {
            if !is_selected(record, options) {
                continue;
            }
            jg.write_start_object()?;
            {
                let mut rw = GribRecordWriter::new(&mut jg, record, options);
                rw.write_header()?;
                if options.print_data {
                    // An unsupported packing template omits the array, exactly
                    // as a null from Grib2Data.getData does in the Java.
                    rw.write_data()?;
                }
            }
            jg.write_end()?;
        }

        jg.write_end()?;
        jg.close()
    }

    /// The NetCDF branch of `write(NetcdfFile, Options)`.
    fn write_netcdf(
        netcdf: &NetcdfFile,
        options: &Options,
        stdout: &mut dyn Write,
    ) -> Result<(), Grib2JsonError> {
        let mut jg = Self::new_json_generator(options, stdout)?;
        jg.write_start_array()?;

        let days = netcdf.read_scalar_int("time").map_err(|e| Grib2JsonError::Oscar(e.to_string()))?;
        // `new DateTime(1992, 10, 5, 0, 0, DateTimeZone.UTC).plusDays(days)`
        let date = DateTime::from_utc(1992, 10, 5, 0, 0).plus_days(days as i64);
        let depth = netcdf
            .read_scalar_double("depth")
            .map_err(|e| Grib2JsonError::Oscar(e.to_string()))?;

        OscarRecordWriter::new(&mut jg, netcdf, "u", date, depth, options).write_record()?;
        OscarRecordWriter::new(&mut jg, netcdf, "v", date, depth, options).write_record()?;

        jg.write_end()?;
        jg.close()?;
        Ok(())
    }

    /// Convert the input file to Json as specified by the command line options.
    pub fn write(&self, stdout: &mut dyn Write) -> Result<(), Grib2JsonError> {
        // Try opening the file as GRIB format.
        let bytes = std::fs::read(&self.file)?;
        let mut input = Grib2Input::new(bytes);
        if input.scan() {
            for options in &self.option_groups {
                Self::write_grib(&input, options, stdout)?;
            }
            Ok(())
        } else {
            // Otherwise, process it as NetCDF format.
            let netcdf =
                NetcdfFile::open(&self.file).map_err(|e| Grib2JsonError::Oscar(e.to_string()))?;
            if self.option_groups.first().is_some_and(|o| o.enable_logging) {
                eprintln!("File contents:\n{}", netcdf.describe());
            }
            for options in &self.option_groups {
                Self::write_netcdf(&netcdf, options, stdout)?;
            }
            Ok(())
        }
    }
}
