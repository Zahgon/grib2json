//! A NetCDF-3 ("classic" / CDF-1 and CDF-2) reader, standing in for the parts
//! of `ucar.nc2.NetcdfFile` that `Grib2Json` and `OscarRecordWriter` use.
//!
//! Only three operations are needed: `findVariable(name)`, `readScalarInt` /
//! `readScalarDouble`, and a strided `read(sectionSpec)` over a float array.
//! OSCAR products ship as classic NetCDF, which is a small, fully specified
//! binary format, so this is a real reader rather than a shim.
//!
//! HDF5-backed NetCDF-4 is *not* handled; [`NetcdfFile::open`] reports that as
//! an error rather than silently misreading. MIGRATION.md records the limit.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// NetCDF external data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NcType {
    Byte,
    Char,
    Short,
    Int,
    Float,
    Double,
}

impl NcType {
    fn from_code(code: u32) -> Option<NcType> {
        Some(match code {
            1 => NcType::Byte,
            2 => NcType::Char,
            3 => NcType::Short,
            4 => NcType::Int,
            5 => NcType::Float,
            6 => NcType::Double,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            NcType::Byte | NcType::Char => 1,
            NcType::Short => 2,
            NcType::Int | NcType::Float => 4,
            NcType::Double => 8,
        }
    }
}

/// One variable's header entry.
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub shape: Vec<usize>,
    pub nc_type: NcType,
    begin: u64,
}

impl Variable {
    /// `Variable.getFullName()`.
    pub fn full_name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug)]
pub enum NetcdfError {
    Io(std::io::Error),
    NotClassic,
    Malformed(&'static str),
    NoSuchVariable(String),
    InvalidRange(String),
}

impl fmt::Display for NetcdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetcdfError::Io(e) => write!(f, "{}", e),
            NetcdfError::NotClassic => {
                write!(f, "not a classic NetCDF file (NetCDF-4/HDF5 is not supported)")
            }
            NetcdfError::Malformed(what) => write!(f, "malformed NetCDF header: {}", what),
            NetcdfError::NoSuchVariable(n) => write!(f, "variable not found: {}", n),
            NetcdfError::InvalidRange(s) => write!(f, "InvalidRangeException: {}", s),
        }
    }
}

impl From<std::io::Error> for NetcdfError {
    fn from(e: std::io::Error) -> Self {
        NetcdfError::Io(e)
    }
}

/// An open classic NetCDF file, held in memory.
pub struct NetcdfFile {
    buf: Vec<u8>,
    vars: HashMap<String, Variable>,
    path: String,
}

/// Cursor over the big-endian header stream.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u32(&mut self) -> Result<u32, NetcdfError> {
        if self.pos + 4 > self.buf.len() {
            return Err(NetcdfError::Malformed("truncated"));
        }
        let v = u32::from_be_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn u64(&mut self) -> Result<u64, NetcdfError> {
        if self.pos + 8 > self.buf.len() {
            return Err(NetcdfError::Malformed("truncated"));
        }
        let v = u64::from_be_bytes(self.buf[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    /// A counted name, padded to a 4-byte boundary.
    fn name(&mut self) -> Result<String, NetcdfError> {
        let n = self.u32()? as usize;
        if self.pos + n > self.buf.len() {
            return Err(NetcdfError::Malformed("truncated name"));
        }
        let s = String::from_utf8_lossy(&self.buf[self.pos..self.pos + n]).into_owned();
        self.pos += n + pad(n);
        Ok(s)
    }

    fn skip(&mut self, n: usize) {
        self.pos += n;
    }
}

/// Bytes of padding needed to reach the next 4-byte boundary.
fn pad(n: usize) -> usize {
    (4 - (n % 4)) % 4
}

const NC_DIMENSION: u32 = 0x0A;
const NC_VARIABLE: u32 = 0x0B;
const NC_ATTRIBUTE: u32 = 0x0C;

impl NetcdfFile {
    /// `NetcdfFile.open(String location)`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<NetcdfFile, NetcdfError> {
        let path_str = path.as_ref().display().to_string();
        let buf = std::fs::read(path.as_ref())?;
        if buf.len() < 8 || &buf[0..3] != b"CDF" {
            return Err(NetcdfError::NotClassic);
        }
        let version = buf[3];
        if version != 1 && version != 2 {
            return Err(NetcdfError::NotClassic);
        }
        let offset64 = version == 2;

        let mut c = Cursor { buf: &buf, pos: 4 };
        let _numrecs = c.u32()?;

        // Dimensions.
        let mut dims: Vec<usize> = Vec::new();
        let tag = c.u32()?;
        let n = c.u32()? as usize;
        if tag == NC_DIMENSION {
            for _ in 0..n {
                let _name = c.name()?;
                dims.push(c.u32()? as usize);
            }
        } else if tag != 0 {
            return Err(NetcdfError::Malformed("expected dimension list"));
        }

        Self::skip_attributes(&mut c)?;

        // Variables.
        let mut vars = HashMap::new();
        let tag = c.u32()?;
        let n = c.u32()? as usize;
        if tag == NC_VARIABLE {
            for _ in 0..n {
                let name = c.name()?;
                let ndims = c.u32()? as usize;
                let mut shape = Vec::with_capacity(ndims);
                for _ in 0..ndims {
                    let id = c.u32()? as usize;
                    shape.push(*dims.get(id).ok_or(NetcdfError::Malformed("bad dimid"))?);
                }
                Self::skip_attributes(&mut c)?;
                let nc_type = NcType::from_code(c.u32()?)
                    .ok_or(NetcdfError::Malformed("unknown nc_type"))?;
                let _vsize = c.u32()?;
                let begin = if offset64 { c.u64()? } else { c.u32()? as u64 };
                vars.insert(name.clone(), Variable { name, shape, nc_type, begin });
            }
        } else if tag != 0 {
            return Err(NetcdfError::Malformed("expected variable list"));
        }

        Ok(NetcdfFile { buf, vars, path: path_str })
    }

    fn skip_attributes(c: &mut Cursor<'_>) -> Result<(), NetcdfError> {
        let tag = c.u32()?;
        let n = c.u32()? as usize;
        if tag == 0 {
            return Ok(());
        }
        if tag != NC_ATTRIBUTE {
            return Err(NetcdfError::Malformed("expected attribute list"));
        }
        for _ in 0..n {
            let _name = c.name()?;
            let ty = NcType::from_code(c.u32()?).ok_or(NetcdfError::Malformed("attr type"))?;
            let nelems = c.u32()? as usize;
            let bytes = nelems * ty.size();
            c.skip(bytes + pad(bytes));
        }
        Ok(())
    }

    /// `NetcdfFile.findVariable(String)`.
    pub fn find_variable(&self, name: &str) -> Option<&Variable> {
        self.vars.get(name)
    }

    fn require(&self, name: &str) -> Result<&Variable, NetcdfError> {
        self.find_variable(name).ok_or_else(|| NetcdfError::NoSuchVariable(name.to_string()))
    }

    /// Read one element of `var` at flat index `i`, widened to f64.
    fn element(&self, var: &Variable, i: usize) -> f64 {
        let off = var.begin as usize + i * var.nc_type.size();
        let b = &self.buf;
        if off + var.nc_type.size() > b.len() {
            return f64::NAN;
        }
        match var.nc_type {
            NcType::Byte | NcType::Char => b[off] as i8 as f64,
            NcType::Short => i16::from_be_bytes([b[off], b[off + 1]]) as f64,
            NcType::Int => i32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]) as f64,
            NcType::Float => {
                f32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]) as f64
            }
            NcType::Double => f64::from_be_bytes([
                b[off], b[off + 1], b[off + 2], b[off + 3],
                b[off + 4], b[off + 5], b[off + 6], b[off + 7],
            ]),
        }
    }

    /// `findVariable(name).readScalarInt()`.
    pub fn read_scalar_int(&self, name: &str) -> Result<i32, NetcdfError> {
        let var = self.require(name)?;
        Ok(self.element(var, 0) as i32)
    }

    /// `findVariable(name).readScalarDouble()`.
    pub fn read_scalar_double(&self, name: &str) -> Result<f64, NetcdfError> {
        let var = self.require(name)?;
        Ok(self.element(var, 0))
    }

    /// `Variable.read(String sectionSpec).reduce()` for a float array.
    ///
    /// `sectionSpec` is ucar's section syntax: comma-separated per-dimension
    /// ranges, each either a single index (`0`) or an inclusive span
    /// (`0:480`). `reduce()` drops the length-1 dimensions, which for a flat
    /// row-major read changes nothing about the element order -- so the result
    /// is returned flat, in the order `IndexIterator` would visit it.
    pub fn read_section(&self, name: &str, spec: &str) -> Result<Vec<f32>, NetcdfError> {
        let var = self.require(name)?.clone();
        let parts: Vec<&str> = spec.split(',').collect();
        if parts.len() != var.shape.len() {
            return Err(NetcdfError::InvalidRange(format!(
                "{} has rank {} but the section names {} dimension(s)",
                name,
                var.shape.len(),
                parts.len()
            )));
        }

        let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(parts.len());
        for (part, &len) in parts.iter().zip(var.shape.iter()) {
            let (first, last) = match part.split_once(':') {
                Some((a, b)) => (parse_index(a, spec)?, parse_index(b, spec)?),
                None => {
                    let i = parse_index(part, spec)?;
                    (i, i)
                }
            };
            if last < first || last >= len {
                return Err(NetcdfError::InvalidRange(format!(
                    "{} out of bounds for dimension of length {}",
                    part, len
                )));
            }
            ranges.push((first, last));
        }

        // Row-major strides for the full variable.
        let rank = var.shape.len();
        let mut strides = vec![1usize; rank];
        for d in (0..rank.saturating_sub(1)).rev() {
            strides[d] = strides[d + 1] * var.shape[d + 1];
        }

        let counts: Vec<usize> = ranges.iter().map(|(a, b)| b - a + 1).collect();
        let total: usize = counts.iter().product();
        let mut out = Vec::with_capacity(total);
        let mut index = vec![0usize; rank];
        for _ in 0..total {
            let flat: usize =
                (0..rank).map(|d| (ranges[d].0 + index[d]) * strides[d]).sum();
            out.push(self.element(&var, flat) as f32);
            // Odometer increment, last dimension fastest.
            for d in (0..rank).rev() {
                index[d] += 1;
                if index[d] < counts[d] {
                    break;
                }
                index[d] = 0;
            }
        }
        Ok(out)
    }

    /// What `log.info("File contents:\n{}", netcdfFile)` prints -- a summary,
    /// used only under `--verbose`.
    pub fn describe(&self) -> String {
        let mut names: Vec<&String> = self.vars.keys().collect();
        names.sort();
        let mut s = format!("netcdf {} {{\n  variables:\n", self.path);
        for n in names {
            let v = &self.vars[n];
            s.push_str(&format!("    {:?} {}{:?};\n", v.nc_type, v.name, v.shape));
        }
        s.push('}');
        s
    }
}

fn parse_index(raw: &str, spec: &str) -> Result<usize, NetcdfError> {
    raw.trim()
        .parse::<usize>()
        .map_err(|_| NetcdfError::InvalidRange(format!("cannot parse section {:?}", spec)))
}
