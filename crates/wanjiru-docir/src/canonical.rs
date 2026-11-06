//! Canonical JSON: the serialisation of RFC 8785 (JSON Canonicalization Scheme).
//!
//! The canonical form is a function of the value alone: no insignificant whitespace, object members
//! sorted by the UTF-16 code units of their names, strings escaped in the one way the RFC allows, and
//! numbers written the way ECMAScript's `Number.prototype.toString` writes them. Two readers that
//! agree on the value therefore agree on the bytes, which is what makes a hash or signature of the
//! text meaningful.

use std::fmt::Write as _;

use wanjiru_math::digest::sha256_hex;
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

use crate::json::Json;

/// The canonical text of `value`. Fails for a number that is NaN or infinite, and for an object that
/// names a key twice.
pub fn to_canonical_string(value: &Json) -> Result<String> {
    let mut out = String::new();
    write_value(value, &mut out)?;
    Ok(out)
}

/// The SHA-256 of the canonical text, as 64 lower-case hexadecimal digits.
pub fn canonical_sha256(value: &Json) -> Result<String> {
    Ok(sha256_hex(to_canonical_string(value)?.as_bytes()))
}

/// The shortest decimal digits that read back to `ax` (positive, finite), as ECMAScript chooses
/// them, with the decimal exponent of the first digit: `ax = d.ddd x 10^exponent`.
///
/// Rust's `{:e}` gives the shortest round-trip digits, but when two shortest candidates are exactly
/// as close to the value (the value is the midpoint between them) it rounds up, whereas ECMAScript
/// takes the even one: `1424953923781206.25` is `...206.2` in ECMAScript and `...206.3` in Rust.
/// Such a tie needs a value with at most 25 fractional bits and at least 15 digits, so only those
/// are re-rounded, from the exact value, to the same number of digits (Rust rounds exact ties to
/// even); the result is used when it still reads back to `ax`.
fn shortest_digits(ax: f64) -> Option<(String, i32)> {
    let mut scientific = format!("{ax:e}");
    let digit_count = scientific
        .split_once('e')?
        .0
        .chars()
        .filter(char::is_ascii_digit)
        .count();
    let scaled = ax * 33_554_432.0; // 2^25, exact unless it overflows
    if digit_count >= 15 && (!scaled.is_finite() || scaled.fract() == 0.0) {
        let exact = format!("{ax:.*e}", digit_count - 1);
        if exact.parse::<f64>() == Ok(ax) {
            scientific = exact;
        }
    }
    let (mantissa, exponent) = scientific.split_once('e')?;
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    Some((
        if digits.is_empty() { "0" } else { digits }.to_owned(),
        exponent.parse().ok()?,
    ))
}

/// `x` as an integer when ECMAScript writes it as that integer's digits. That holds for every integer up
/// to 2^53 in magnitude: neighbouring doubles are at most 1 apart there, so no shorter digit string reads
/// back to `x`. Beyond 2^53 the shortest digits can differ from the exact ones, and the general path
/// decides. NaN and the infinities are not integers.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the range check makes the conversion exact"
)]
fn small_integer(x: f64) -> Option<i64> {
    (x.fract() == 0.0 && x.abs() <= 9_007_199_254_740_992.0).then_some(x as i64)
}

/// A finite number as ECMAScript writes it (`Number::toString`, radix 10): the shortest digit string
/// that reads back to the same double, laid out in plain or exponent notation by the rules of the
/// ECMAScript specification. Negative zero is written `0`.
pub fn format_number(x: f64) -> Result<String> {
    if let Some(n) = small_integer(x) {
        return Ok(n.to_string());
    }
    format_general(x)
}

/// [`format_number`] without the shortcut for integers: the path every other number takes.
fn format_general(x: f64) -> Result<String> {
    if !x.is_finite() {
        return Err(WnjError::new(
            codes::JSON_NUMBER,
            ErrorKind::InvalidInput,
            "NaN and infinities have no JSON representation",
        ));
    }
    if x == 0.0 {
        return Ok("0".to_owned());
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let internal = || {
        WnjError::new(
            codes::JSON_NUMBER,
            ErrorKind::Internal,
            format!("cannot find the digits of {x:e}"),
        )
    };
    let (digits, exponent) = shortest_digits(x.abs()).ok_or_else(internal)?;
    // The value is 0.DIGITS x 10^n with k digits.
    let k = i32::try_from(digits.len()).map_err(|_| internal())?;
    let n = exponent + 1;
    let zeros = |count: i32| "0".repeat(usize::try_from(count).unwrap_or_default());
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", zeros(n - k))
    } else if 0 < n && n <= 21 {
        let (whole, fraction) = digits.split_at(usize::try_from(n).unwrap_or_default());
        format!("{whole}.{fraction}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", zeros(-n))
    } else {
        let e = n - 1;
        let e_sign = if e < 0 { '-' } else { '+' };
        let (first, rest) = digits.split_at(1);
        if rest.is_empty() {
            format!("{first}e{e_sign}{}", e.abs())
        } else {
            format!("{first}.{rest}e{e_sign}{}", e.abs())
        }
    };
    Ok(format!("{sign}{body}"))
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(value: &Json, out: &mut String) -> Result<()> {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Number(x) => match small_integer(*x) {
            Some(n) => {
                let _ = write!(out, "{n}");
            }
            None => out.push_str(&format_general(*x)?),
        },
        Json::String(s) => write_string(s, out),
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out)?;
            }
            out.push(']');
        }
        Json::Object(members) => {
            let mut sorted: Vec<&(String, Json)> = members.iter().collect();
            // RFC 8785 section 3.2.3: by the UTF-16 code units of the names, not by code point or byte.
            sorted.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            if let Some(pair) = sorted.windows(2).find(|w| w[0].0 == w[1].0) {
                return Err(WnjError::new(
                    codes::JSON_DUPLICATE_KEY,
                    ErrorKind::InvalidInput,
                    format!("object names the key {:?} twice", pair[0].0),
                ));
            }
            out.push('{');
            for (i, (key, member)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(member, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn number(bits: u64) -> String {
        format_number(f64::from_bits(bits)).expect("finite")
    }

    #[test]
    fn rfc_8785_appendix_b_number_vectors() {
        // IEEE-754 bit patterns and the text RFC 8785 gives for them (each also confirmed with Node's
        // `String(x)`, an independent ECMAScript implementation).
        let vectors: [(u64, &str); 24] = [
            (0x0000_0000_0000_0000, "0"),
            (0x8000_0000_0000_0000, "0"),
            (0x0000_0000_0000_0001, "5e-324"),
            (0x8000_0000_0000_0001, "-5e-324"),
            (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
            (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
            (0x4340_0000_0000_0000, "9007199254740992"),
            (0xc340_0000_0000_0000, "-9007199254740992"),
            (0x4430_0000_0000_0000, "295147905179352830000"),
            (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
            (0x44b5_2d02_c7e1_4af6, "1e+23"),
            (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
            (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
            (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
            (0x444b_1ae4_d6e2_ef50, "1e+21"),
            (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
            (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
            (0x41b3_de43_5555_5553, "333333333.3333332"),
            (0x41b3_de43_5555_5554, "333333333.33333325"),
            (0x41b3_de43_5555_5555, "333333333.3333333"),
            (0x41b3_de43_5555_5556, "333333333.3333334"),
            (0x41b3_de43_5555_5557, "333333333.33333343"),
            (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
            (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
        ];
        for (bits, text) in vectors {
            assert_eq!(number(bits), text, "bits {bits:016x}");
        }
    }

    #[test]
    fn layout_rules_around_the_switch_to_exponent_notation() {
        for (x, text) in [
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (123.456, "123.456"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (0.000_001, "0.000001"),
            (0.000_000_1, "1e-7"),
            (1.25e-7, "1.25e-7"),
            (1e100, "1e+100"),
            (255.0, "255"),
        ] {
            assert_eq!(format_number(x).expect("finite"), text, "{x:e}");
        }
        assert!(format_number(f64::NAN).is_err());
        assert!(format_number(f64::INFINITY).is_err());
        assert!(format_number(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn strings_use_the_one_permitted_escaping() {
        let value = Json::string("a\"b\\c\u{8}\u{c}\n\r\t\u{1}\u{1f}/\u{7f}é\u{2028}😀");
        assert_eq!(
            to_canonical_string(&value).expect("string"),
            "\"a\\\"b\\\\c\\b\\f\\n\\r\\t\\u0001\\u001f/\u{7f}é\u{2028}😀\""
        );
    }

    #[test]
    fn members_are_sorted_by_utf16_code_units() {
        // RFC 8785 section 3.2.3: U+1F600 (surrogates D83D DE00) sorts before U+FB33, although its
        // code point is larger.
        let input = parse(
            "{\"\\u20ac\":\"Euro Sign\",\"\\r\":\"Carriage Return\",\"\\ufb33\":\"Hebrew Letter Dalet With Dagesh\",\
             \"1\":\"One\",\"\\ud83d\\ude00\":\"Emoji: Grinning Face\",\"\\u0080\":\"Control\",\
             \"\\u00f6\":\"Latin Small Letter O With Diaeresis\"}",
        )
        .expect("valid");
        assert_eq!(
            to_canonical_string(&input).expect("canonical"),
            "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\
             \"ö\":\"Latin Small Letter O With Diaeresis\",\"€\":\"Euro Sign\",\
             \"😀\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}"
        );
    }

    #[test]
    fn whole_value_example() {
        let input = parse(
            "{\"numbers\":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001],\
             \"string\":\"\\u20ac$\\u000F\\u000aA'\\u0042\\u0022\\u005c\\\\\\\"\\/\",\"literals\":[null,true,false]}",
        )
        .expect("valid");
        assert_eq!(
            to_canonical_string(&input).expect("canonical"),
            "{\"literals\":[null,true,false],\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],\
             \"string\":\"€$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}"
        );
    }

    #[test]
    fn duplicate_members_and_non_finite_numbers_are_refused() {
        let dup = Json::Object(vec![("a".into(), Json::Null), ("a".into(), Json::Null)]);
        assert_eq!(
            to_canonical_string(&dup)
                .expect_err("dup")
                .code()
                .to_string(),
            "WNJ-E1002"
        );
        let nan = Json::Array(vec![Json::Number(f64::NAN)]);
        assert_eq!(
            to_canonical_string(&nan)
                .expect_err("nan")
                .code()
                .to_string(),
            "WNJ-E1007"
        );
    }

    #[test]
    fn canonical_form_is_a_fixed_point_and_hashes_it() {
        let text = "{\"b\":[1,2.5,\"x\"],\"a\":{\"z\":null,\"y\":true}}";
        let once = to_canonical_string(&parse(text).expect("valid")).expect("canonical");
        let twice = to_canonical_string(&parse(&once).expect("valid")).expect("canonical");
        assert_eq!(once, "{\"a\":{\"y\":true,\"z\":null},\"b\":[1,2.5,\"x\"]}");
        assert_eq!(once, twice);
        let hash = canonical_sha256(&parse(text).expect("valid")).expect("hash");
        assert_eq!(hash, sha256_hex(once.as_bytes()));
        assert_eq!(hash.len(), 64);
    }

    /// The shortcut for integers must never change a byte: compare it with the general path on every
    /// integer near zero, on both sides of 2^53 and on powers of two and of ten.
    #[test]
    fn the_integer_shortcut_agrees_with_the_general_path() {
        let mut values = vec![
            0.0,
            -0.0,
            1.5,
            -2.5,
            9_007_199_254_740_991.0,
            9_007_199_254_740_992.0,
            -9_007_199_254_740_992.0,
            9_007_199_254_740_994.0,
            1e21,
            1e22,
        ];
        values.extend((-5_000..5_000).map(f64::from));
        let mut power = 1.0;
        for _ in 0..80 {
            values.extend([power, -power, power - 1.0, power + 1.0]);
            power *= 2.0;
        }
        let mut power = 1.0;
        for _ in 0..24 {
            values.extend([power, -power, power - 1.0, power + 1.0]);
            power *= 10.0;
        }
        for x in values {
            assert_eq!(
                format_number(x).expect("finite"),
                format_general(x).expect("finite"),
                "{x:e}"
            );
            let mut written = String::new();
            write_value(&Json::Number(x), &mut written).expect("finite");
            assert_eq!(written, format_general(x).expect("finite"), "{x:e}");
        }
    }
}
