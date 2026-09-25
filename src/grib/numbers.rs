//! Port of `ucar.grib.GribNumbers` -- the sentinels and bit helpers the writers use.

/// `GribNumbers.UNDEFINED`.
///
/// ucar substitutes this for any integer field whose octets are all-ones
/// ("missing" in the GRIB2 spec). `AbstractRecordWriter.writeIfSet` compares
/// against it, which is what keeps `subDivisions`, `spLon`, `np` and friends out
/// of the header when a template does not carry them.
pub const UNDEFINED: i32 = -9999;

/// `GribNumbers.UNDEFINEDD`.
///
/// The double-valued counterpart, used for `levelValue1` / `levelValue2`. It is
/// written unconditionally by `writeProduct`, so it shows up in real output as
/// `"surface2Value":-9.999E-252` whenever the second fixed surface is missing.
pub const UNDEFINED_D: f64 = -9.999E-252;

/// `GribNumbers.BIT_1` .. `BIT_8` -- masks numbered from the most significant bit,
/// which is how the GRIB2 spec numbers flag bits.
pub const BIT_1: i32 = 128;
pub const BIT_2: i32 = 64;
pub const BIT_3: i32 = 32;
pub const BIT_4: i32 = 16;
pub const BIT_5: i32 = 8;
pub const BIT_6: i32 = 4;
pub const BIT_7: i32 = 2;
pub const BIT_8: i32 = 1;

/// `GribNumbers.isBitSet(int value, int bit)`.
pub fn is_bit_set(value: i32, bit: i32) -> bool {
    (value & bit) != 0
}

/// Read an unsigned big-endian integer of `n` octets, mapping the all-ones
/// "missing" encoding to [`UNDEFINED`], exactly as ucar's readers do.
pub fn uint(buf: &[u8], off: usize, n: usize) -> i32 {
    if off + n > buf.len() {
        return UNDEFINED;
    }
    let mut v: u64 = 0;
    let mut all_ones = true;
    for i in 0..n {
        let b = buf[off + i];
        if b != 0xFF {
            all_ones = false;
        }
        v = (v << 8) | b as u64;
    }
    if all_ones {
        UNDEFINED
    } else {
        v as i32
    }
}

/// Read a single octet as a literal code, with no "missing" mapping.
///
/// Code-table fields are not numeric measurements: an all-ones octet is a real
/// entry in the table, not an absent value. Type of second fixed surface 255 is
/// `"Missing"` in code table 4.5 and netCDF-Java reports it as 255 -- mapping it
/// to [`UNDEFINED`] would print `-9999` / `"Unknown"` instead.
pub fn code(buf: &[u8], off: usize) -> i32 {
    match buf.get(off) {
        Some(b) => *b as i32,
        None => UNDEFINED,
    }
}

/// Read a *signed* GRIB integer: the top bit is a sign flag, the rest magnitude.
///
/// GRIB2 does not use two's complement for scale factors and scaled values.
pub fn int(buf: &[u8], off: usize, n: usize) -> i32 {
    if off + n > buf.len() {
        return UNDEFINED;
    }
    let mut all_ones = true;
    for i in 0..n {
        if buf[off + i] != 0xFF {
            all_ones = false;
        }
    }
    if all_ones {
        return UNDEFINED;
    }
    let negative = buf[off] & 0x80 != 0;
    let mut v: i64 = (buf[off] & 0x7F) as i64;
    for i in 1..n {
        v = (v << 8) | buf[off + i] as i64;
    }
    if negative {
        -v as i32
    } else {
        v as i32
    }
}

/// Read a big-endian IEEE-754 single.
pub fn float(buf: &[u8], off: usize) -> f32 {
    if off + 4 > buf.len() {
        return UNDEFINED as f32;
    }
    f32::from_bits(
        ((buf[off] as u32) << 24)
            | ((buf[off + 1] as u32) << 16)
            | ((buf[off + 2] as u32) << 8)
            | (buf[off + 3] as u32),
    )
}

/// Read a 4-octet IBM-format ("GRIB") float.
///
/// Not IEEE-754. GRIB inherited the System/360 hexadecimal format for some
/// floating fields, and netCDF-Java reads them that way:
///
/// ```text
/// sign     = octet 0, bit 7
/// exponent = octet 0, bits 0-6, excess-64, base 16
/// mantissa = octets 1-3, a 24-bit fraction
/// value    = (-1)^sign * mantissa * 16^(exponent - 64) / 2^24
/// ```
///
/// This is not guesswork. Feeding the Java build IEEE bit patterns through a
/// rotated lat/lon grid produced 0x40400000 -> 0.25, 0x41800000 -> 8.0,
/// 0x42c80000 -> 200.0, 0x3f800000 -> 0.03125 and 0xc0200000 -> -0.125 --
/// every one of which the formula above reproduces and IEEE-754 does not.
/// Section 5's reference value is a genuine IEEE single, so the two readers
/// coexist; see [`float`].
pub fn ibm_float(buf: &[u8], off: usize) -> f32 {
    if off + 4 > buf.len() {
        return UNDEFINED as f32;
    }
    let a = buf[off];
    let sign = if a & 0x80 != 0 { -1.0f64 } else { 1.0f64 };
    let exponent = (a & 0x7F) as i32 - 64;
    let mantissa =
        (((buf[off + 1] as u32) << 16) | ((buf[off + 2] as u32) << 8) | buf[off + 3] as u32) as f64;
    (sign * mantissa * 16f64.powi(exponent) / 16_777_216.0) as f32
}

/// Read an unsigned big-endian 8-octet length (section 0's total message length).
pub fn ulong(buf: &[u8], off: usize) -> i64 {
    let mut v: i64 = 0;
    for i in 0..8 {
        v = (v << 8) | buf[off + i] as i64;
    }
    v
}
