//! Validators for field values: is this string even a well-formed amount, date, IBAN or MRZ?
//!
//! The harness uses them for two things: to refuse a corpus whose ground truth contains a malformed
//! value (a wrong label silently lowers every score), and to count predictions that were accepted
//! although they violate a hard rule, which is an error no confidence threshold can excuse.
//!
//! These functions are written separately from the parsers in `wanjiru-fields` (Phase 4) on purpose:
//! a check that shares code with the thing it checks only finds disagreements between two copies of
//! the same mistake. `tools/oracles/make_eval_vectors.py` cross-checks them against an independent
//! Python implementation.

use std::ops::Range;

use wanjiru_docir::FieldKind;

/// Check `value` against the syntax of `kind`; the error says what is wrong.
pub fn validate(kind: FieldKind, value: &str) -> Result<(), String> {
    match kind {
        FieldKind::Amount => amount(value),
        FieldKind::Date => date(value),
        FieldKind::Iban => iban(value),
        FieldKind::Mrz => mrz(value),
        FieldKind::Text => {
            if value.is_empty() {
                Err("a text value is not empty".to_owned())
            } else {
                Ok(())
            }
        }
    }
}

/// A canonical decimal amount: an optional minus sign, an integer part without leading zeros, an
/// optional fraction, at most 18 digits in all, no sign for zero, no separators, no exponent.
fn amount(value: &str) -> Result<(), String> {
    let body = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = match body.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (body, None),
    };
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(whole) {
        return Err("the integer part is one or more ASCII digits".to_owned());
    }
    if whole.len() > 1 && whole.starts_with('0') {
        return Err("the integer part has no leading zero".to_owned());
    }
    if let Some(f) = fraction {
        if !all_digits(f) {
            return Err("the fraction is one or more ASCII digits after a point".to_owned());
        }
    }
    if whole.len() + fraction.map_or(0, str::len) > 18 {
        return Err("at most 18 digits".to_owned());
    }
    let is_zero = whole == "0" && fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'));
    if is_zero && value.starts_with('-') {
        return Err("zero has no sign".to_owned());
    }
    Ok(())
}

const fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A calendar date written `YYYY-MM-DD`, in the years 1900 to 2100.
fn date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err("a date is YYYY-MM-DD".to_owned());
    }
    let number = |range: Range<usize>| -> Option<u32> {
        let digits = &bytes[range];
        digits.iter().all(u8::is_ascii_digit).then(|| {
            digits
                .iter()
                .fold(0_u32, |acc, d| acc * 10 + u32::from(d - b'0'))
        })
    };
    let (Some(year), Some(month), Some(day)) = (number(0..4), number(5..7), number(8..10)) else {
        return Err("a date is YYYY-MM-DD with ASCII digits".to_owned());
    };
    if !(1900..=2100).contains(&year) {
        return Err("the year is between 1900 and 2100".to_owned());
    }
    if !(1..=12).contains(&month) {
        return Err("the month is 01 to 12".to_owned());
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err(format!(
            "the day is 01 to {} in {year}-{month:02}",
            days_in_month(year, month)
        ));
    }
    Ok(())
}

/// An IBAN in compact form: upper-case letters and digits, 15 to 34 characters, two letters, two check
/// digits, and the ISO 7064 mod 97-10 check. Country-specific lengths are not checked.
fn iban(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if !(15..=34).contains(&bytes.len()) {
        return Err("an IBAN has 15 to 34 characters".to_owned());
    }
    if !bytes
        .iter()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Err("an IBAN is upper-case letters and digits without spaces".to_owned());
    }
    if !(bytes[0].is_ascii_uppercase()
        && bytes[1].is_ascii_uppercase()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit())
    {
        return Err("an IBAN starts with two letters and two digits".to_owned());
    }
    // Move the first four characters to the end, read letters as 10..35, and reduce mod 97 digit by digit.
    let mut remainder = 0_u32;
    for &b in bytes[4..].iter().chain(&bytes[..4]) {
        if b.is_ascii_digit() {
            remainder = (remainder * 10 + u32::from(b - b'0')) % 97;
        } else {
            let v = u32::from(b - b'A') + 10;
            remainder = (remainder * 100 + v) % 97;
        }
    }
    if remainder == 1 {
        Ok(())
    } else {
        Err("the mod 97 check fails".to_owned())
    }
}

/// ICAO 9303 check digit of `field`: weights 7, 3, 1 repeating; digits are themselves, letters are
/// 10 to 35, the filler `<` is 0. `None` for any other character.
#[must_use]
pub fn mrz_check_digit(field: &[u8]) -> Option<u8> {
    const WEIGHTS: [u32; 3] = [7, 3, 1];
    let mut sum = 0_u32;
    for (i, &b) in field.iter().enumerate() {
        let value = match b {
            b'0'..=b'9' => u32::from(b - b'0'),
            b'A'..=b'Z' => u32::from(b - b'A') + 10,
            b'<' => 0,
            _ => return None,
        };
        sum += value * WEIGHTS[i % 3];
    }
    u8::try_from(sum % 10).ok()
}

/// Compare the check digit computed over `data` with the character found in the zone.
fn check_digit_matches(data: &[u8], found: u8, what: &str) -> Result<(), String> {
    let expected = mrz_check_digit(data).ok_or_else(|| format!("{what}: invalid character"))?;
    if found == b'0' + expected {
        Ok(())
    } else {
        Err(format!("{what}: check digit should be {expected}"))
    }
}

fn check_field(line: &[u8], data: Range<usize>, check_at: usize, what: &str) -> Result<(), String> {
    check_digit_matches(&line[data], line[check_at], what)
}

/// A machine-readable zone as its lines joined by `\n`: TD1 (3 x 30), TD2 (2 x 36) or TD3 (2 x 44)
/// characters of `A-Z`, `0-9` and `<`, with every ICAO 9303 check digit correct. The name line carries
/// no check digit and is not examined beyond its length and alphabet. Document numbers longer than
/// nine characters (TD1 check digit `<`) are not supported.
fn mrz(value: &str) -> Result<(), String> {
    let lines: Vec<&[u8]> = value.split('\n').map(str::as_bytes).collect();
    let valid_char = |b: &u8| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'<';
    if !lines.iter().all(|l| l.iter().all(valid_char)) {
        return Err("an MRZ uses A-Z, 0-9 and < only".to_owned());
    }
    let shape: Vec<usize> = lines.iter().map(|l| l.len()).collect();
    match shape.as_slice() {
        [44, 44] => td3(lines[1]),
        [36, 36] => td2(lines[1]),
        [30, 30, 30] => td1(lines[0], lines[1]),
        _ => Err("an MRZ is 2 lines of 44, 2 of 36 or 3 of 30 characters".to_owned()),
    }
}

/// The composite check digit of a one-line-per-record zone: the digit at `check_at` covers the listed
/// parts of the same line, concatenated.
fn composite(line: &[u8], parts: &[Range<usize>], check_at: usize) -> Result<(), String> {
    let joined: Vec<u8> = parts
        .iter()
        .flat_map(|r| line[r.clone()].iter().copied())
        .collect();
    check_digit_matches(&joined, line[check_at], "composite")
}

fn td3(line: &[u8]) -> Result<(), String> {
    check_field(line, 0..9, 9, "document number")?;
    check_field(line, 13..19, 19, "date of birth")?;
    check_field(line, 21..27, 27, "date of expiry")?;
    let personal = &line[28..42];
    let personal_ok = personal.iter().all(|b| *b == b'<') && matches!(line[42], b'<' | b'0');
    if !personal_ok {
        check_field(line, 28..42, 42, "personal number")?;
    }
    composite(line, &[0..10, 13..20, 21..43], 43)
}

fn td2(line: &[u8]) -> Result<(), String> {
    check_field(line, 0..9, 9, "document number")?;
    check_field(line, 13..19, 19, "date of birth")?;
    check_field(line, 21..27, 27, "date of expiry")?;
    composite(line, &[0..10, 13..20, 21..35], 35)
}

fn td1(first: &[u8], second: &[u8]) -> Result<(), String> {
    if first[14] == b'<' {
        return Err("document numbers longer than nine characters are not supported".to_owned());
    }
    check_field(first, 5..14, 14, "document number")?;
    check_field(second, 0..6, 6, "date of birth")?;
    check_field(second, 8..14, 14, "date of expiry")?;
    // The composite digit covers line 1 positions 5-29 and line 2 positions 0-6, 8-14 and 18-28.
    let mut joined: Vec<u8> = first[5..30].to_vec();
    for range in [0..7, 8..15, 18..29] {
        joined.extend_from_slice(&second[range]);
    }
    check_digit_matches(&joined, second[29], "composite")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(kind: FieldKind, value: &str) {
        validate(kind, value).unwrap_or_else(|e| panic!("{value:?} should be valid: {e}"));
    }

    fn bad(kind: FieldKind, value: &str) {
        assert!(
            validate(kind, value).is_err(),
            "{value:?} should be invalid"
        );
    }

    #[test]
    fn amounts() {
        for good in [
            "0",
            "0.00",
            "1",
            "1234.50",
            "-12.5",
            "100000000000000000",
            "0.000001",
            "7.0",
        ] {
            ok(FieldKind::Amount, good);
        }
        for wrong in [
            "",
            "-",
            ".5",
            "5.",
            "01",
            "-0",
            "-0.00",
            "+5",
            "1,234.50",
            "1 234",
            "1e3",
            "12.3.4",
            "１２",
            "1234567890123456789",
        ] {
            bad(FieldKind::Amount, wrong);
        }
    }

    #[test]
    fn dates_follow_the_calendar() {
        for good in [
            "2026-10-29",
            "2024-02-29",
            "2000-02-29",
            "1900-01-01",
            "2100-12-31",
            "2026-04-30",
        ] {
            ok(FieldKind::Date, good);
        }
        for wrong in [
            "2026-02-29",
            "1900-02-29",
            "2100-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026-4-3",
            "26-10-29",
            "2026/10/29",
            "1899-12-31",
            "2101-01-01",
            "",
            "2026-10-2x",
        ] {
            bad(FieldKind::Date, wrong);
        }
    }

    #[test]
    fn ibans_use_mod_97() {
        for good in [
            "DE89370400440532013000",
            "GB82WEST12345698765432",
            "FR1420041010050500013M02606",
            "NL91ABNA0417164300",
            "CH9300762011623852957",
            "BE68539007547034",
            "NO9386011117947",
            "MT84MALT011000012345MTLCAST001S",
        ] {
            ok(FieldKind::Iban, good);
        }
        for wrong in [
            "DE89370400440532013001",
            "GB82WEST12345698765433",
            "de89370400440532013000",
            "DE89 3704 0044 0532 0130 00",
            "DE8937040044053201300",
            "DE",
            "1289370400440532013000",
            "DEXX370400440532013000",
            "",
        ] {
            bad(FieldKind::Iban, wrong);
        }
    }

    #[test]
    fn check_digit_of_the_icao_specimen_fields() {
        assert_eq!(mrz_check_digit(b"L898902C3"), Some(6));
        assert_eq!(mrz_check_digit(b"740812"), Some(2));
        assert_eq!(mrz_check_digit(b"120415"), Some(9));
        assert_eq!(mrz_check_digit(b"ZE184226B<<<<<"), Some(1));
        assert_eq!(mrz_check_digit(b""), Some(0));
        assert_eq!(mrz_check_digit(b"a"), None);
    }

    #[test]
    fn td3_specimen_and_corruptions() {
        let line1 = "P<UTOERIKSSON<<ANNA<MARIA<<<<<<<<<<<<<<<<<<<";
        let line2 = "L898902C36UTO7408122F1204159ZE184226B<<<<<10";
        ok(FieldKind::Mrz, &format!("{line1}\n{line2}"));
        // Flip one character in each checked region: every flip must be caught.
        for position in [0, 5, 9, 13, 18, 19, 21, 26, 27, 30, 42, 43] {
            let mut flipped: Vec<u8> = line2.bytes().collect();
            flipped[position] = if flipped[position] == b'7' {
                b'3'
            } else {
                b'7'
            };
            let text = format!("{line1}\n{}", String::from_utf8(flipped).expect("ASCII"));
            bad(FieldKind::Mrz, &text);
        }
        bad(FieldKind::Mrz, line2);
        bad(
            FieldKind::Mrz,
            &format!("{line1}\n{}", line2.to_lowercase()),
        );
        bad(FieldKind::Mrz, &format!("{line1}\n{line2}x"));
    }
}
