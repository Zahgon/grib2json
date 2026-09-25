//! Java text formatting primitives that the output depends on.
//!
//! Port of the three JDK / joda-time behaviours that reach the JSON stream:
//! `Float.toString`, `Double.toString`, and `org.joda.time.DateTime.toString()`
//! for a UTC instant. Every rule below was measured against the Java build
//! (`maven:3.9-eclipse-temurin-17`); the captures are reproduced in
//! `tests/jtext_test.rs`.

/// `Float.toString(float)`.
///
/// The JDK contract: for `1e-3 <= |v| < 1e7` print plain decimal, otherwise
/// computerized scientific notation `d.dddEn`; in both forms there is always at
/// least one digit after the decimal point, and beyond that exactly as many
/// digits selected by the Java 17 conversion algorithm (including its legacy
/// interval bounds and integer-rounding behavior).
///
/// `NaN` / `Infinity` are handled here for completeness, but
/// [`crate::float_value::FloatValue`] intercepts them before they arrive.
pub fn float_to_string(value: f32) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == 0.0 {
        // Java distinguishes the two zeros; `is_sign_negative` is the only way
        // to see -0.0 here because -0.0 == 0.0 is true.
        return if value.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let neg = value.is_sign_negative();
    let sci = java_float_scientific(value.abs());
    render_java_number(&sci, neg)
}

/// A bounded unsigned integer for exact binary32 decimal conversion. Four limbs
/// leave ample room for the 151-bit denominator and decimal digit extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FloatInteger([u64; 4]);
impl FloatInteger {
    fn new(value: u64) -> Self { Self([value, 0, 0, 0]) }
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
    fn times(self, multiplier: u64) -> Self {
        let mut out = Self::new(0);
        let mut carry = 0u128;
        for i in 0..4 {
            let product = self.0[i] as u128 * multiplier as u128 + carry;
            out.0[i] = product as u64;
            carry = product >> 64;
        }
        assert_eq!(carry, 0, "binary32 decimal conversion overflow");
        out
    }
    fn shift(mut self, bits: u32) -> Self {
        for _ in 0..bits { self = self.times(2); }
        self
    }
    fn plus(self, other: Self) -> Self {
        let mut out = Self::new(0);
        let mut carry = 0u128;
        for i in 0..4 {
            let sum = self.0[i] as u128 + other.0[i] as u128 + carry;
            out.0[i] = sum as u64;
            carry = sum >> 64;
        }
        assert_eq!(carry, 0);
        out
    }
    fn subtract(&mut self, other: Self) {
        let mut borrow = false;
        for i in 0..4 {
            let (value, first) = self.0[i].overflowing_sub(other.0[i]);
            let (value, second) = value.overflowing_sub(u64::from(borrow));
            self.0[i] = value;
            borrow = first || second;
        }
        assert!(!borrow);
    }
}

/// Generate Java 17's finite binary32 decimal digits using exact rational
/// intervals. The legacy formatter uses a symmetric half-ULP interval, narrows
/// it once more at powers of two, and retains an integer-rounding shortcut.
/// These rules explain why a generic shortest-roundtrip formatter disagrees at
/// subnormals, the minimum normal, and some large integral floats.
fn java_float_scientific(value: f32) -> String {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let bits = value.to_bits();
    let exponent_bits = (bits >> 23) & 255;
    let significand = (bits & 0x7fffff) | if exponent_bits == 0 { 0 } else { 1 << 23 };
    let binary_power = if exponent_bits == 0 { -149 } else { exponent_bits as i32 - 150 };
    let highest_power = binary_power + 31 - significand.leading_zeros() as i32;

    // The original formatter emits integral values through integer arithmetic
    // when they fit in a signed long. It drops only whole decimal positions
    // which are below the represented binary precision.
    if highest_power <= 62 && binary_power + significand.trailing_zeros() as i32 >= 0 {
        let integer = value as u64;
        let insignificant = if highest_power > 24 {
            (1u64 << (highest_power - 25)).ilog10()
        } else { 0 };
        let scale = 10u64.pow(insignificant);
        let rounded = integer / scale + u64::from(integer % scale >= (scale + 1) / 2);
        let text = rounded.to_string();
        let exponent = text.len() as i32 - 1 + insignificant as i32;
        let digits = text.trim_end_matches('0');
        return format!("{}.{}e{}", &digits[..1], &digits[1..], exponent);
    }

    // Scale the exact value and its interval into compact integers. Matching
    // the legacy decimal-exponent estimate matters at decade boundaries: an
    // overestimate can retain a leading zero until the final rounding step.
    let significant_bits = if exponent_bits == 0 { 32 - significand.leading_zeros() } else { 24 };
    let compact = significand >> significand.trailing_zeros();
    let fraction_bits = 32 - compact.leading_zeros();
    let tiny = (fraction_bits as i32 - highest_power - 1).max(0);
    let normalized = (significand as f64) / 2f64.powi(31 - significand.leading_zeros() as i32);
    let mut exponent = ((normalized - 1.5) * 0.289529654 + 0.176091259
        + highest_power as f64 * 0.301029995663981).floor() as i32;
    let numerator_fives = (-exponent).max(0);
    let denominator_fives = exponent.max(0);
    let mut numerator_twos = numerator_fives + tiny + highest_power - fraction_bits as i32 + 1;
    let mut denominator_twos = denominator_fives + tiny;
    let mut margin_twos = numerator_fives + tiny + highest_power - significant_bits as i32;
    let common = numerator_twos.min(denominator_twos);
    numerator_twos -= common;
    denominator_twos -= common;
    margin_twos -= common + i32::from(compact == 1);
    if margin_twos < 0 {
        numerator_twos -= margin_twos;
        denominator_twos -= margin_twos;
        margin_twos = 0;
    }
    fn scaled_integer(value: u64, fives: i32, twos: i32) -> FloatInteger {
        let mut result = FloatInteger::new(value);
        for _ in 0..fives { result = result.times(5); }
        result.shift(twos as u32)
    }
    let mut numerator = scaled_integer(compact as u64, numerator_fives, numerator_twos);
    let denominator = scaled_integer(1, denominator_fives, denominator_twos);
    let mut margin = scaled_integer(1, numerator_fives, margin_twos);
    let ten_denominator = denominator.times(10);
    fn five_bits(power: i32) -> i32 {
        if power == 0 { 0 } else if power <= 26 { (64 - 5u64.pow(power as u32).leading_zeros()) as i32 }
        else { power * 3 }
    }
    let value_width = fraction_bits as i32 + numerator_twos + five_bits(numerator_fives);
    let denominator_width = denominator_twos + 1 + five_bits(denominator_fives + 1);
    let fixed_width = if value_width < 32 && denominator_width < 32 { 32 }
        else if value_width < 64 && denominator_width < 64 { 64 } else { 0 };
    // Java's historical int/long paths use signed arithmetic for stopping and
    // final rounding. Preserve its overflow behavior, which affects some large
    // finite floats. The unbounded path instead includes its upper boundary.
    fn signed(value: u64, width: i32) -> i64 {
        if width == 32 { value as i32 as i64 } else { value as i64 }
    }
    let mut digits = Vec::new();
    loop {
        let mut digit = 0u8;
        while numerator.cmp(&denominator) != Less {
            numerator.subtract(denominator);
            digit += 1;
        }
        assert!(digit < 10);
        numerator = numerator.times(10);
        margin = margin.times(10);
        let (low, high, middle) = if fixed_width == 0 {
            (numerator.cmp(&margin) == Less,
             numerator.plus(margin).cmp(&ten_denominator) != Less,
             numerator.times(2).cmp(&ten_denominator))
        } else {
            let b = numerator.0[0];
            let m = margin.0[0];
            let ten = ten_denominator.0[0];
            let overflowed_margin = !digits.is_empty() && signed(m, fixed_width) <= 0;
            (overflowed_margin || signed(b, fixed_width) < signed(m, fixed_width),
             overflowed_margin || signed(b.wrapping_add(m), fixed_width) > signed(ten, fixed_width),
             signed(b.wrapping_mul(2).wrapping_sub(ten), fixed_width).cmp(&0))
        };
        let first = digits.is_empty();
        if first && digit == 0 && !high {
            exponent -= 1;
        } else {
            digits.push(digit);
        }
        let need_fraction = first && (exponent < -3 || exponent >= 8);
        if (low || high) && !need_fraction {
            if high && (!low || middle == Greater || (middle == Equal && digit % 2 != 0)) {
                let mut i = digits.len();
                loop {
                    i -= 1;
                    if digits[i] < 9 { digits[i] += 1; break; }
                    digits[i] = 0;
                    if i == 0 { digits[0] = 1; exponent += 1; break; }
                }
            }
            break;
        }
    }
    let digits: String = digits.into_iter().map(|d| (b'0' + d) as char).collect();
    format!("{}.{}e{}", &digits[..1], &digits[1..], exponent)
}

/// `Double.toString(double)`. Same rules, same thresholds, wider mantissa.
pub fn double_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let neg = value.is_sign_negative();
    let sci = format!("{:e}", value.abs());
    render_java_number(&sci, neg)
}

/// Lower bound of the plain-decimal window, as a base-10 exponent.
const PLAIN_MIN_EXP: i32 = -3;
/// Upper bound (exclusive) of the plain-decimal window.
const PLAIN_MAX_EXP: i32 = 7;

/// Turn Rust's `d.ddde<exp>` into Java's chosen presentation.
fn render_java_number(sci: &str, neg: bool) -> String {
    let (mantissa, exp) = sci.split_once('e').expect("{:e} always emits an exponent");
    let exp: i32 = exp.parse().expect("{:e} always emits a decimal exponent");
    // Digits with the point removed: "1.23456" -> "123456", "1" -> "1".
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();

    let mut out = String::with_capacity(digits.len() + 8);
    if neg {
        out.push('-');
    }
    if (PLAIN_MIN_EXP..PLAIN_MAX_EXP).contains(&exp) {
        if exp >= 0 {
            let int_len = (exp + 1) as usize;
            if digits.len() <= int_len {
                out.push_str(&digits);
                // 1e2 -> digits "1", exp 2: the integer part needs padding.
                for _ in digits.len()..int_len {
                    out.push('0');
                }
                out.push_str(".0");
            } else {
                out.push_str(&digits[..int_len]);
                out.push('.');
                out.push_str(&digits[int_len..]);
            }
        } else {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        out.push('E');
        out.push_str(&exp.to_string());
    }
    out
}

/// Days in each month of a non-leap year.
const MONTH_DAYS: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// A civil date-time, UTC, to millisecond resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub millis: i64,
}

impl DateTime {
    /// From milliseconds since the Unix epoch, the units `Grib2IdentificationSection.getRefTime()` returns.
    pub fn from_millis(millis: i64) -> Self {
        DateTime { millis }
    }

    /// From a UTC civil date-time, mirroring `new DateTime(y, m, d, h, mi, DateTimeZone.UTC)`.
    pub fn from_utc(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> Self {
        let mut days = 0i64;
        if year >= 1970 {
            for y in 1970..year {
                days += if is_leap(y) { 366 } else { 365 };
            }
        } else {
            for y in year..1970 {
                days -= if is_leap(y) { 366 } else { 365 };
            }
        }
        for m in 1..month {
            days += MONTH_DAYS[(m - 1) as usize];
            if m == 2 && is_leap(year) {
                days += 1;
            }
        }
        days += day - 1;
        DateTime { millis: ((days * 24 + hour) * 60 + minute) * 60 * 1000 }
    }

    /// `DateTime.plusDays(int)`.
    pub fn plus_days(self, days: i64) -> Self {
        DateTime { millis: self.millis + days * 86_400_000 }
    }

    /// Civil `(year, month, day, hour, minute, second, milli)` in UTC.
    pub fn fields(self) -> (i64, i64, i64, i64, i64, i64, i64) {
        // Floor division, so instants before 1970 decompose correctly.
        let mut day = self.millis.div_euclid(86_400_000);
        let mut rem = self.millis.rem_euclid(86_400_000);
        let milli = rem % 1000;
        rem /= 1000;
        let second = rem % 60;
        rem /= 60;
        let minute = rem % 60;
        let hour = rem / 60;

        let mut year = 1970i64;
        loop {
            let len = if is_leap(year) { 366 } else { 365 };
            if day >= len {
                day -= len;
                year += 1;
            } else if day < 0 {
                year -= 1;
                day += if is_leap(year) { 366 } else { 365 };
            } else {
                break;
            }
        }
        let mut month = 1i64;
        loop {
            let mut len = MONTH_DAYS[(month - 1) as usize];
            if month == 2 && is_leap(year) {
                len += 1;
            }
            if day >= len {
                day -= len;
                month += 1;
            } else {
                break;
            }
        }
        (year, month, day + 1, hour, minute, second, milli)
    }

    /// `dateTime.withZone(DateTimeZone.UTC).toString()`.
    ///
    /// joda's default printer is `ISODateTimeFormat.dateTime()`, which always
    /// emits milliseconds and renders the UTC offset as a literal `Z`.
    pub fn to_iso_string(self) -> String {
        let (y, mo, d, h, mi, s, ms) = self.fields();
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", y, mo, d, h, mi, s, ms)
    }
}

#[cfg(test)]
mod float_compatibility_tests {
    use super::float_to_string;
    #[test]
    fn java17_boundary_and_rounding_captures() {
        // Independently captured with Float.toString in Eclipse Temurin 17.
        // Cover interval selection, decade carry, integral precision and the
        // historical signed-arithmetic path, in addition to negative values.
        for (bits, expected) in [
            (0x00000001, "1.4E-45"),
            (0x00000002, "2.8E-45"),
            (0x00000047, "1.0E-43"),
            (0x00800000, "1.17549435E-38"),
            (0x68fffffb, "9.6714036E24"),
            (0x69000000, "9.6714065E24"),
            (0x6a400001, "5.8028443E25"),
            (0x7f7fffff, "3.4028235E38"),
            (0x80000001, "-1.4E-45"),
            (0x80000000, "-0.0"),
        ] {
            assert_eq!(float_to_string(f32::from_bits(bits)), expected, "{bits:08x}");
        }
    }
}
