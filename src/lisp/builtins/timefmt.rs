//! GNU nstrftime modifiers, independent of host strftime extensions.
//! libc supplies only locale names/composites/eras; numeric fields, widths,
//! signs and case flags must behave the same on macOS and Linux.
use super::misc::Tm;

#[derive(Clone, Copy, PartialEq)]
enum Pad {
    Default,
    Zero,
    Sign,
    Space,
    None,
}

fn padded(mut value: String, width: usize, pad: Pad, numeric: bool) -> Result<String, ()> {
    if pad == Pad::None || width <= value.len() {
        return Ok(value);
    }
    let count = width - value.len();
    value.try_reserve(count).map_err(|_| ())?;
    let zero = matches!(pad, Pad::Zero | Pad::Sign) || (numeric && pad == Pad::Default);
    let at = if numeric && zero && value.starts_with(['-', '+']) {
        1
    } else {
        0
    };
    // Reserve before constructing padding, so impossible widths return a Lisp
    // error instead of overflowing a size calculation or a Rust allocation.
    let mut padding = String::new();
    padding.try_reserve(count).map_err(|_| ())?;
    padding.extend(std::iter::repeat_n(if zero { '0' } else { ' ' }, count));
    value.insert_str(at, &padding);
    Ok(value)
}

fn number(
    value: i64,
    digits: usize,
    width: Option<usize>,
    pad: Pad,
    year: bool,
) -> Result<String, ()> {
    let width = width.unwrap_or(digits);
    let sign = year
        && pad == Pad::Sign
        && value >= 0
        && (value >= if digits == 2 { 100 } else { 10000 } || width > digits);
    padded(
        if sign {
            format!("+{value}")
        } else {
            value.to_string()
        },
        width,
        pad,
        true,
    )
}

pub(super) fn format_time(
    fmt: &str,
    tm: &Tm,
    seconds: i64,
    nanos: u32,
    zone: &str,
    native: &mut impl FnMut(&str) -> Option<String>,
) -> Result<String, ()> {
    let mut out = String::new();
    let mut k = 0;
    let bytes = fmt.as_bytes();
    while k < bytes.len() {
        if bytes[k] != b'%' {
            let c = fmt[k..].chars().next().unwrap();
            out.push(c);
            k += c.len_utf8();
            continue;
        }
        let start = k;
        k += 1;
        let mut pad = Pad::Default;
        let mut upper = false;
        let mut change_case = false;
        while let Some(&c) = bytes.get(k) {
            match c {
                b'_' => pad = Pad::Space,
                b'-' => pad = Pad::None,
                b'0' => pad = Pad::Zero,
                b'+' => pad = Pad::Sign,
                b'^' => upper = true,
                b'#' => change_case = true,
                _ => break,
            }
            k += 1;
        }
        let mut width: Option<usize> = None;
        while let Some(&c) = bytes.get(k).filter(|c| c.is_ascii_digit()) {
            let n = width
                .unwrap_or(0)
                .checked_mul(10)
                .and_then(|n| n.checked_add((c - b'0') as usize))
                .ok_or(())?;
            if n > isize::MAX as usize {
                return Err(());
            }
            width = Some(n);
            k += 1;
        }
        let modifier = match bytes.get(k) {
            Some(b'E' | b'O') => {
                let c = bytes[k];
                k += 1;
                c
            }
            _ => 0,
        };
        let mut spec = fmt[k..].chars().next();
        let mut colons = 0;
        if spec == Some(':') {
            let mut j = k;
            while bytes.get(j) == Some(&b':') {
                j += 1;
            }
            if bytes.get(j) == Some(&b'z') {
                colons = j - k;
                k = j;
                spec = Some('z');
            }
        }
        if let Some(c) = spec {
            k += c.len_utf8();
        }
        // A modified % is invalid and its final % starts another conversion.
        let percent = spec == Some('%');
        if (percent && k - start > 2) || spec == Some('\0') {
            k -= 1;
        }
        if spec == Some('\0') {
            spec = None;
        }
        // nstrftime consumes one byte for an invalid conversion, even when
        // that byte begins UTF-8. Keep Rust text intact while excluding the
        // continuation bytes from the directive's padding calculation.
        let literal_suffix = spec
            .filter(|c| !c.is_ascii())
            .map_or(0, |c| c.len_utf8() - 1);
        let raw = &fmt[start..k];
        let valid_modifier = match modifier {
            b'E' => spec.is_some_and(|c| "cnpqrstuxyzCPRTXYZ".contains(c)),
            b'O' => spec.is_some_and(|c| "bdeghjklmnpqrstuwyzBCGHIMNPRSTUVWYZ".contains(c)),
            _ => true,
        };
        let spec = if valid_modifier && colons <= 3 {
            spec
        } else {
            None
        };
        let year = tm.tm_year as i64 + 1900;
        let hour12 = (tm.tm_hour + 11) % 12 + 1;
        let mut numeric = None;
        let mut text = None;
        let mut already_padded = false;
        match spec {
            Some('C' | 'y' | 'Y') if modifier == b'E' => {
                let Some(value) = native(&format!("%E{}", spec.unwrap())) else {
                    continue;
                };
                text = Some(value);
            }
            Some('C') if year < 0 => {
                text = Some(padded(
                    format!("-{}", year.unsigned_abs() / 100),
                    width.unwrap_or(2),
                    pad,
                    true,
                )?);
                already_padded = true;
            }
            Some('C') => numeric = Some((year / 100, 2, true)),
            Some('Y') => numeric = Some((year, 4, true)),
            Some('y') => numeric = Some((year.abs() % 100, 2, true)),
            Some('d' | 'e') => numeric = Some((tm.tm_mday as i64, 2, false)),
            Some('H' | 'k') => numeric = Some((tm.tm_hour as i64, 2, false)),
            Some('I' | 'l') => numeric = Some((hour12 as i64, 2, false)),
            Some('j') => numeric = Some((tm.tm_yday as i64 + 1, 3, false)),
            Some('m') => numeric = Some((tm.tm_mon as i64 + 1, 2, false)),
            Some('M') => numeric = Some((tm.tm_min as i64, 2, false)),
            Some('S') => numeric = Some((tm.tm_sec as i64, 2, false)),
            Some('q') => numeric = Some((tm.tm_mon as i64 / 3 + 1, 1, false)),
            Some('s') => numeric = Some((seconds, 1, false)),
            Some('w') => numeric = Some((tm.tm_wday as i64, 1, false)),
            Some('u') => numeric = Some((((tm.tm_wday + 6) % 7 + 1) as i64, 1, false)),
            Some('U') => numeric = Some((((tm.tm_yday - tm.tm_wday + 7) / 7) as i64, 2, false)),
            Some('W') => {
                numeric = Some((
                    ((tm.tm_yday - (tm.tm_wday + 6) % 7 + 7) / 7) as i64,
                    2,
                    false,
                ))
            }
            Some('g' | 'G' | 'V') => {
                let leap = |y: i64| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
                let iso_days = |d: i32| d - (d - tm.tm_wday + 4).rem_euclid(7) + 3;
                let mut days = iso_days(tm.tm_yday);
                let mut iso_year = year;
                if days < 0 {
                    iso_year -= 1;
                    days = iso_days(tm.tm_yday + 365 + i32::from(leap(iso_year)));
                } else {
                    let next = iso_days(tm.tm_yday - 365 - i32::from(leap(year)));
                    if next >= 0 {
                        iso_year += 1;
                        days = next;
                    }
                }
                numeric = Some(match spec.unwrap() {
                    'g' => (iso_year.abs() % 100, 2, true),
                    'G' => (iso_year, 4, true),
                    _ => ((days / 7 + 1) as i64, 2, false),
                });
            }
            Some('F') => {
                let (yp, yw) = if pad == Pad::Default && width.is_none() {
                    (Pad::Sign, 4)
                } else {
                    (pad, width.unwrap_or(0).saturating_sub(6))
                };
                text = Some(format!(
                    "{}-{:02}-{:02}",
                    number(year, 4, Some(yw), yp, true)?,
                    tm.tm_mon + 1,
                    tm.tm_mday
                ));
                already_padded = true;
            }
            Some('D') => {
                text = Some(format!(
                    "{:02}/{:02}/{}",
                    tm.tm_mon + 1,
                    tm.tm_mday,
                    number(year.abs() % 100, 2, None, pad, true)?
                ))
            }
            Some('R') => text = Some(format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)),
            Some('T') => {
                text = Some(format!(
                    "{:02}:{:02}:{:02}",
                    tm.tm_hour, tm.tm_min, tm.tm_sec
                ))
            }
            Some('N') => {
                let w = width.filter(|w| *w > 0).unwrap_or(9);
                let s = format!("{nanos:09}");
                let mut s = s[..w.min(9)].trim_end_matches('0').to_owned();
                if s.is_empty() {
                    s.push('0');
                }
                // Fractional precision pads on the right, not the left.
                if pad != Pad::None && w > s.len() {
                    let count = w - s.len();
                    s.try_reserve(count).map_err(|_| ())?;
                    s.extend(std::iter::repeat_n(
                        if pad == Pad::Space { ' ' } else { '0' },
                        count,
                    ));
                }
                text = Some(s);
                already_padded = true;
            }
            Some('z') => {
                let offset = tm.tm_gmtoff;
                let a = offset.unsigned_abs();
                let (h, m, s) = (a / 3600, a / 60 % 60, a % 60);
                let colons = if colons == 3 {
                    if s != 0 {
                        2
                    } else if m != 0 {
                        1
                    } else {
                        3
                    }
                } else {
                    colons
                };
                let (body, digits) = match colons {
                    0 => (format!("{}", h * 100 + m), 5),
                    1 => (format!("{h}:{m:02}"), 6),
                    2 => (format!("{h}:{m:02}:{s:02}"), 9),
                    _ => (h.to_string(), 3),
                };
                let sign = if offset < 0 || (offset == 0 && zone.starts_with('-')) {
                    '-'
                } else {
                    '+'
                };
                text = Some(padded(
                    format!("{sign}{body}"),
                    width.unwrap_or(digits),
                    pad,
                    true,
                )?);
                already_padded = true;
            }
            Some('Z') => text = Some(zone.to_owned()),
            Some('n') => text = Some("\n".into()),
            Some('t') => text = Some("\t".into()),
            Some('%') if raw == "%%" => text = Some("%".into()),
            // Darwin strftime can return an empty %r. GNU expands the
            // portable twelve-hour clock there while retaining locale %p.
            #[cfg(target_os = "macos")]
            Some('r') => {
                text = Some(format!(
                    "{hour12:02}:{:02}:{:02} {}",
                    tm.tm_min,
                    tm.tm_sec,
                    native("%p").unwrap_or_default()
                ));
            }
            Some('a' | 'A' | 'b' | 'B' | 'h' | 'c' | 'x' | 'X' | 'p' | 'P' | 'r') => {
                let c = match spec.unwrap() {
                    'P' => 'p',
                    'h' => 'b',
                    c => c,
                };
                let m = if modifier == b'E' && "cxX".contains(c) {
                    "E"
                } else if modifier == b'O' && "bB".contains(c) {
                    "O"
                } else {
                    ""
                };
                // Failed native conversion emits nothing, without width
                // padding. A successful empty locale string still pads.
                let Some(value) = native(&format!("%{m}{c}")) else {
                    continue;
                };
                text = Some(value);
            }
            _ => text = Some(raw.to_owned()),
        }
        let mut value = if let Some((value, digits, year)) = numeric {
            let p = if pad == Pad::Default && matches!(spec, Some('e' | 'k' | 'l')) {
                Pad::Space
            } else {
                pad
            };
            already_padded = true;
            number(value, digits, width, p, year)?
        } else {
            text.unwrap()
        };
        let lower = spec == Some('P') || (change_case && matches!(spec, Some('p' | 'Z')));
        if lower {
            value.make_ascii_lowercase();
        } else if upper || (change_case && matches!(spec, Some('a' | 'A' | 'b' | 'B' | 'h'))) {
            value.make_ascii_uppercase();
        }
        if !already_padded {
            let width = width
                .map(|w| w.checked_add(literal_suffix).ok_or(()))
                .transpose()?
                .unwrap_or(0);
            value = padded(value, width, pad, false)?;
        }
        out.try_reserve(value.len()).map_err(|_| ())?;
        out.push_str(&value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_modifiers_do_not_reach_libc() {
        let tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 1,
            tm_mon: 0,
            tm_year: 70,
            tm_wday: 4,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        let mut native = |spec: &str| match spec {
            "%B" => Some("January".to_owned()),
            "%p" => Some("AM".to_owned()),
            _ => panic!("nonportable conversion reached libc: {spec}"),
        };
        for (fmt, expected) in [
            ("%5Y", "01970"),
            ("%03d", "001"),
            ("%-_5Y", " 1970"),
            ("%_-5Y", "1970"),
            ("%10B", "   January"),
            ("%0#10B", "000JANUARY"),
            ("%_5P", "   am"),
            ("%8Z", "     UTC"),
            ("%_8:z", "   +0:00"),
            ("%1d", "1"),
            ("%-3N", "0"),
            ("%12N", "000000000000"),
            ("%5é", "  %5é"),
            ("%05é", "0%05é"),
            ("%8😀", "     %8😀"),
            ("%5\0Y", "   %5\0Y"),
        ] {
            assert_eq!(
                format_time(fmt, &tm, 0, 0, "UTC", &mut native).unwrap(),
                expected,
                "{fmt}"
            );
        }
        let mut native_status = |spec: &str| {
            if spec == "%c" {
                None
            } else {
                Some(String::new())
            }
        };
        assert_eq!(
            format_time("%010c", &tm, 0, 0, "UTC", &mut native_status).unwrap(),
            ""
        );
        assert_eq!(
            format_time("%010p", &tm, 0, 0, "UTC", &mut native_status).unwrap(),
            "0000000000"
        );
        assert_eq!(
            format_time("%1024B", &tm, 0, 0, "UTC", &mut native)
                .unwrap()
                .len(),
            1024
        );
        assert!(format_time("%999999999999999999999Y", &tm, 0, 0, "UTC", &mut native).is_err());
    }
}
