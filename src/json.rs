//! Port of the `javax.json.stream.JsonGenerator` behaviour grib2json relies on.
//!
//! Only the subset the Java program calls is implemented, but that subset is
//! byte-exact against `org.glassfish:javax.json:1.0.3`, both compact and with
//! `JsonGenerator.PRETTY_PRINTING` enabled. The layout rules below were
//! measured from the Java build rather than inferred; `tests/json_test.rs`
//! reproduces the captures.
//!
//! The pretty printer has three properties that are easy to get wrong and that
//! a "looks like pretty JSON" reimplementation would miss:
//!
//! 1. **A newline is written *before* every item, including the first.** The
//!    stream therefore *opens* with `\n` -- the very first byte of a pretty
//!    document is a line break, not `[`.
//! 2. **There is no space after the `:`** in a name/value pair, in either mode.
//! 3. **An empty structure still spans two lines**: `{` then a newline then the
//!    closing `}` at the parent indent.
//!
//! Neither mode emits a trailing newline at the end of the document.

use crate::float_value::FloatValue;
use crate::jtext::{double_to_string, float_to_string};
use std::io::{self, Write};

/// Indent unit used by the glassfish pretty printer.
const INDENT: &str = "    ";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Array,
    Object,
}

/// A streaming Json writer.
pub struct JsonGenerator<W: Write> {
    out: W,
    pretty: bool,
    stack: Vec<Scope>,
    /// Whether the structure currently on top of the stack already holds an item,
    /// which is what decides if the next item needs a leading comma.
    non_empty: Vec<bool>,
}

impl<W: Write> JsonGenerator<W> {
    /// `Json.createGeneratorFactory(config).createGenerator(out)`.
    ///
    /// `pretty` corresponds to `JsonGenerator.PRETTY_PRINTING`; the Java code
    /// passes `null` config for compact output, which is `pretty = false`.
    pub fn new(out: W, pretty: bool) -> Self {
        JsonGenerator { out, pretty, stack: Vec::new(), non_empty: Vec::new() }
    }

    /// Comma for a sibling, then the newline + indent the pretty printer owes
    /// this item. Called immediately before every value and every structure.
    fn before_item(&mut self) -> io::Result<()> {
        if let Some(last) = self.non_empty.last_mut() {
            if *last {
                self.out.write_all(b",")?;
            } else {
                *last = true;
            }
        }
        if self.pretty {
            self.out.write_all(b"\n")?;
            for _ in 0..self.stack.len() {
                self.out.write_all(INDENT.as_bytes())?;
            }
        }
        Ok(())
    }

    /// Newline + indent owed to a closing bracket, at the *parent* depth.
    fn before_close(&mut self) -> io::Result<()> {
        if self.pretty {
            self.out.write_all(b"\n")?;
            for _ in 0..self.stack.len().saturating_sub(1) {
                self.out.write_all(INDENT.as_bytes())?;
            }
        }
        Ok(())
    }

    fn push(&mut self, scope: Scope) {
        self.stack.push(scope);
        self.non_empty.push(false);
    }

    /// `writeStartArray()`.
    pub fn write_start_array(&mut self) -> io::Result<()> {
        self.before_item()?;
        self.out.write_all(b"[")?;
        self.push(Scope::Array);
        Ok(())
    }

    /// `writeStartArray(String name)`.
    pub fn write_start_array_named(&mut self, name: &str) -> io::Result<()> {
        self.before_item()?;
        self.write_quoted(name)?;
        self.out.write_all(b":[")?;
        self.push(Scope::Array);
        Ok(())
    }

    /// `writeStartObject()`.
    pub fn write_start_object(&mut self) -> io::Result<()> {
        self.before_item()?;
        self.out.write_all(b"{")?;
        self.push(Scope::Object);
        Ok(())
    }

    /// `writeStartObject(String name)`.
    pub fn write_start_object_named(&mut self, name: &str) -> io::Result<()> {
        self.before_item()?;
        self.write_quoted(name)?;
        self.out.write_all(b":{")?;
        self.push(Scope::Object);
        Ok(())
    }

    /// `writeEnd()`.
    pub fn write_end(&mut self) -> io::Result<()> {
        self.before_close()?;
        let scope = self.stack.pop().expect("writeEnd without a matching start");
        self.non_empty.pop();
        self.out.write_all(match scope {
            Scope::Array => b"]",
            Scope::Object => b"}",
        })
    }

    /// `write(String name, <value>)` for a name/value pair carrying raw Json text.
    fn write_pair_raw(&mut self, name: &str, raw: &str) -> io::Result<()> {
        self.before_item()?;
        self.write_quoted(name)?;
        self.out.write_all(b":")?;
        self.out.write_all(raw.as_bytes())
    }

    /// `write(String name, int value)`.
    pub fn write_int(&mut self, name: &str, value: i32) -> io::Result<()> {
        self.write_pair_raw(name, &value.to_string())
    }

    /// `write(String name, long value)`.
    pub fn write_long(&mut self, name: &str, value: i64) -> io::Result<()> {
        self.write_pair_raw(name, &value.to_string())
    }

    /// `write(String name, double value)`.
    pub fn write_double(&mut self, name: &str, value: f64) -> io::Result<()> {
        self.write_pair_raw(name, &double_to_string(value))
    }

    /// `write(String name, JsonNumber value)` where the number is a [`FloatValue`].
    pub fn write_float_value(&mut self, name: &str, value: FloatValue) -> io::Result<()> {
        self.write_pair_raw(name, &value.to_json_text())
    }

    /// `write(String name, String value)`.
    pub fn write_string(&mut self, name: &str, value: &str) -> io::Result<()> {
        self.before_item()?;
        self.write_quoted(name)?;
        self.out.write_all(b":")?;
        self.write_quoted(value)
    }

    /// `write(JsonNumber value)` inside an array, where the number is a [`FloatValue`].
    pub fn write_float_value_item(&mut self, value: FloatValue) -> io::Result<()> {
        self.before_item()?;
        self.out.write_all(value.to_json_text().as_bytes())
    }

    /// `write(JsonValue.NULL)` inside an array.
    pub fn write_null_item(&mut self) -> io::Result<()> {
        self.before_item()?;
        self.out.write_all(b"null")
    }

    /// `write(float)` inside an array, for completeness of the generator surface.
    pub fn write_float_item(&mut self, value: f32) -> io::Result<()> {
        self.before_item()?;
        self.out.write_all(float_to_string(value).as_bytes())
    }

    /// `close()` -- flush; the caller owns the sink's lifetime, as in Java.
    pub fn close(&mut self) -> io::Result<()> {
        self.out.flush()
    }

    /// A Json string literal, escaped the way the glassfish writer escapes.
    ///
    /// The five short escapes plus `\uXXXX` for the remaining C0 controls.
    /// Note what is *not* escaped: `/` is written through, and so is every
    /// non-ASCII character -- the Java generator emits `é` and `中` raw rather
    /// than as surrogate escapes.
    fn write_quoted(&mut self, s: &str) -> io::Result<()> {
        self.out.write_all(b"\"")?;
        let mut buf = String::with_capacity(s.len() + 2);
        for c in s.chars() {
            match c {
                '"' => buf.push_str("\\\""),
                '\\' => buf.push_str("\\\\"),
                '\u{8}' => buf.push_str("\\b"),
                '\u{c}' => buf.push_str("\\f"),
                '\n' => buf.push_str("\\n"),
                '\r' => buf.push_str("\\r"),
                '\t' => buf.push_str("\\t"),
                c if (c as u32) < 0x20 => buf.push_str(&format!("\\u{:04x}", c as u32)),
                c => buf.push(c),
            }
        }
        self.out.write_all(buf.as_bytes())?;
        self.out.write_all(b"\"")
    }
}

/// Render a closure's generator output to a `String`, for tests and for the
/// `--output` path's in-memory checks.
pub fn to_string<F>(pretty: bool, f: F) -> io::Result<String>
where
    F: FnOnce(&mut JsonGenerator<&mut Vec<u8>>) -> io::Result<()>,
{
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut jg = JsonGenerator::new(&mut buf, pretty);
        f(&mut jg)?;
        jg.close()?;
    }
    Ok(String::from_utf8(buf).expect("generator only writes UTF-8"))
}
