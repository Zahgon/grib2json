//! Port of `ucar.grib.grib2.Grib2Data.getData(...)` -- unpacking section 7.

use super::record::Grib2Record;

/// Why a record's data could not be produced.
///
/// `Grib2Data.getData` returns `null` for a record it cannot unpack, and
/// `GribRecordWriter.writeData` skips the `data` array when it does. The same
/// contract holds here: [`get_data`] returns `None`, the writer omits the key,
/// and the reason is available for the caller to log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// The message carried no section 5.
    NoDataRepresentation,
    /// A packing template this decoder does not implement.
    Template(i32),
    /// Section 7 held fewer octets than the template requires.
    Truncated,
}

/// Data representation template 5.0 -- simple packing.
const SIMPLE_PACKING: i32 = 0;

/// `Grib2Data.getData(gdsOffset, pdsOffset, refTime)`.
///
/// Simple packing reconstitutes each value as
/// `Y = (R + X * 2^E) / 10^D`, where `X` is the next `numberOfBits`-wide
/// unsigned field in the section 7 bit stream.
///
/// When a bitmap is present, points the bitmap clears are not stored in section
/// 7 at all; they come back as `NaN`, which `FloatValue` then renders as the
/// Json string `"NaN"`.
pub fn get_data(record: &Grib2Record) -> Result<Vec<f32>, Unsupported> {
    let drs = record.drs.as_ref().ok_or(Unsupported::NoDataRepresentation)?;
    if drs.template != SIMPLE_PACKING {
        return Err(Unsupported::Template(drs.template));
    }

    let nbits = drs.number_of_bits.max(0) as usize;
    let count = drs.number_of_values.max(0) as usize;
    let reference = drs.reference_value as f64;
    let binary_scale = 2f64.powi(drs.binary_scale_factor);
    let decimal_scale = 10f64.powi(drs.decimal_scale_factor);

    // A width of zero means every point holds the reference value; the bit
    // stream is empty and reading from it would be wrong rather than merely
    // wasteful.
    let unpacked: Vec<f32> = if nbits == 0 {
        let v = (reference / decimal_scale) as f32;
        vec![v; count]
    } else {
        if record.data_bytes.len() * 8 < count * nbits {
            return Err(Unsupported::Truncated);
        }
        let mut reader = BitReader::new(&record.data_bytes);
        (0..count)
            .map(|_| {
                let x = reader.read(nbits) as f64;
                ((reference + x * binary_scale) / decimal_scale) as f32
            })
            .collect()
    };

    match &record.bitmap {
        None => Ok(unpacked),
        Some(bits) => {
            // The bitmap has one bit per grid point; a set bit consumes the next
            // unpacked value, a clear bit yields a missing one.
            let total = record.gds.number_points.max(0) as usize;
            let mut out = Vec::with_capacity(total);
            let mut next = unpacked.into_iter();
            for i in 0..total {
                let byte = bits.get(i / 8).copied().unwrap_or(0);
                let present = (byte >> (7 - (i % 8))) & 1 == 1;
                out.push(if present { next.next().unwrap_or(f32::NAN) } else { f32::NAN });
            }
            Ok(out)
        }
    }
}

/// Big-endian MSB-first bit stream reader, matching how GRIB packs fields.
struct BitReader<'a> {
    buf: &'a [u8],
    bit: usize,
}

impl<'a> BitReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        BitReader { buf, bit: 0 }
    }

    /// Read the next `n` bits as an unsigned integer. `n` is at most 64.
    fn read(&mut self, n: usize) -> u64 {
        let mut v: u64 = 0;
        for _ in 0..n {
            let byte = self.buf.get(self.bit / 8).copied().unwrap_or(0);
            let b = (byte >> (7 - (self.bit % 8))) & 1;
            v = (v << 1) | b as u64;
            self.bit += 1;
        }
        v
    }
}
