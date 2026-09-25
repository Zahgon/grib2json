//! Port of `net.nullschool.grib2json.AbstractRecordWriter`.
//!
//! Java expresses this as an abstract base class whose subclasses inherit the
//! overloaded `write` helpers. Rust has no inheritance, so the shared state and
//! the helpers live in [`RecordWriter`] and the two concrete writers hold one.
//! The method names and the dispatch are unchanged, so the writer bodies remain
//! a line-for-line transcription of the Java.

use crate::float_value::FloatValue;
use crate::grib::numbers::UNDEFINED;
use crate::json::JsonGenerator;
use crate::options::Options;
use std::io::{self, Write};

/// The `jg` + `options` pair every record writer is constructed with.
pub struct RecordWriter<'a, W: Write> {
    pub jg: &'a mut JsonGenerator<W>,
    pub options: &'a Options,
}

impl<'a, W: Write> RecordWriter<'a, W> {
    /// `AbstractRecordWriter(JsonGenerator, Options)`.
    ///
    /// Java calls `Objects.requireNonNull` on both; in Rust a `&mut` reference
    /// cannot be null, so the check has no runtime counterpart to carry over.
    pub fn new(jg: &'a mut JsonGenerator<W>, options: &'a Options) -> Self {
        RecordWriter { jg, options }
    }

    /// Write a `"key":int` Json pair.
    pub fn write_int(&mut self, key: &str, value: i32) -> io::Result<()> {
        self.jg.write_int(key, value)
    }

    /// Write a `"key":int` Json pair only if the value is not `GribNumbers.UNDEFINED`.
    pub fn write_int_if_set(&mut self, key: &str, value: i32) -> io::Result<()> {
        if value != UNDEFINED {
            self.jg.write_int(key, value)?;
        }
        Ok(())
    }

    /// Write a `"key":long` Json pair.
    pub fn write_long(&mut self, key: &str, value: i64) -> io::Result<()> {
        self.jg.write_long(key, value)
    }

    /// Write a `"key":float` Json pair.
    pub fn write_float(&mut self, key: &str, value: f32) -> io::Result<()> {
        self.jg.write_float_value(key, FloatValue::new(value))
    }

    /// Write a `"key":float` Json pair only if the value is not `GribNumbers.UNDEFINED`.
    ///
    /// The Java comparison is `value != UNDEFINED` with `UNDEFINED` an `int`,
    /// so the constant is promoted to `float` and the test is against
    /// `-9999.0f`. Comparing against `UNDEFINED as f32` reproduces that,
    /// including for a genuine measurement that happens to be exactly -9999.
    pub fn write_float_if_set(&mut self, key: &str, value: f32) -> io::Result<()> {
        if value != UNDEFINED as f32 {
            self.jg.write_float_value(key, FloatValue::new(value))?;
        }
        Ok(())
    }

    /// Write a `"key":double` Json pair.
    pub fn write_double(&mut self, key: &str, value: f64) -> io::Result<()> {
        self.jg.write_double(key, value)
    }

    /// Write a `"key":"value"` Json pair.
    pub fn write_str(&mut self, key: &str, value: &str) -> io::Result<()> {
        self.jg.write_string(key, value)
    }

    /// Write a `"key":"value"` Json pair, and a second `"keyName":"name"` pair
    /// if the command line options have name printing enabled.
    pub fn write_named(&mut self, key: &str, code: i32, name: &str) -> io::Result<()> {
        self.write_int(key, code)?;
        if self.options.print_names {
            let key_name = format!("{}Name", key);
            self.write_str(&key_name, name)?;
        }
        Ok(())
    }
}
