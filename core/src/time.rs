//! Small date helpers, so the engine needs no date library.
//! Everything is UTC; exports that give local times without a zone are read as UTC.

/// Unix milliseconds for a calendar date and time (UTC).
pub fn civil_to_ms(y: i64, m: i64, d: i64, hh: i64, mm: i64, ss: i64) -> i64 {
    // Howard Hinnant's days-from-civil algorithm.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    ((days * 24 + hh) * 60 + mm) * 60_000 + ss * 1000
}

fn num(s: &str) -> Option<i64> {
    s.trim().parse().ok()
}

/// "2021-03-04 05:06:07", "2021-03-04T05:06:07.123Z", "2021-03-04 05:06:07 UTC"
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 10 {
        return None;
    }
    let (y, m, d) = (num(s.get(0..4)?)?, num(s.get(5..7)?)?, num(s.get(8..10)?)?);
    let (hh, mm, ss) = if s.len() >= 19 {
        (num(s.get(11..13)?)?, num(s.get(14..16)?)?, num(s.get(17..19)?)?)
    } else {
        (0, 0, 0)
    };
    Some(civil_to_ms(y, m, d, hh, mm, ss))
}

/// X/Twitter style: "Wed Oct 10 20:19:24 +0000 2018"
pub fn parse_twitter(s: &str) -> Option<i64> {
    let p: Vec<&str> = s.split_whitespace().collect();
    if p.len() < 6 {
        return parse_iso(s);
    }
    let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let m = months.iter().position(|x| *x == p[1])? as i64 + 1;
    let t: Vec<i64> = p[3].split(':').filter_map(num).collect();
    if t.len() != 3 {
        return None;
    }
    Some(civil_to_ms(num(p[5])?, m, num(p[2])?, t[0], t[1], t[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates() {
        assert_eq!(civil_to_ms(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(parse_iso("2021-03-04 05:06:07 UTC"), Some(1_614_834_367_000));
        assert_eq!(parse_iso("2021-03-04T05:06:07.000Z"), Some(1_614_834_367_000));
        assert_eq!(parse_twitter("Thu Mar 04 05:06:07 +0000 2021"), Some(1_614_834_367_000));
        assert_eq!(parse_iso("2000-02-29"), Some(951_782_400_000));
    }
}
