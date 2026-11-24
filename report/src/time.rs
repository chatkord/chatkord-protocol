//! UTC dates and clock times from Unix seconds, so a report names no time zone and takes no date library.

const DAY: u64 = 86_400;

/// `HH:MM:SS` of the day `seconds` falls in, or `--:--:--` when the clock gave none.
pub(crate) fn clock(seconds: Option<u64>) -> String {
    let Some(seconds) = seconds else { return "--:--:--".to_owned() };
    let of_day = seconds % DAY;
    format!("{:02}:{:02}:{:02}", of_day / 3600, of_day % 3600 / 60, of_day % 60)
}

/// `YYYY-MM-DD HH:MM UTC`.
pub(crate) fn stamp(seconds: u64) -> String {
    let (year, month, day) = civil(seconds / DAY);
    let of_day = seconds % DAY;
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", of_day / 3600, of_day % 3600 / 60)
}

/// The proleptic Gregorian date of a day count since 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let of_era = shifted % 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let day_of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 { month_part + 3 } else { month_part - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_match_known_instants() {
        assert_eq!(stamp(0), "1970-01-01 00:00 UTC");
        assert_eq!(stamp(1_763_376_120), "2025-11-17 10:42 UTC");
        assert_eq!(stamp(951_782_400), "2000-02-29 00:00 UTC");
        assert_eq!(clock(Some(1_763_376_122)), "10:42:02");
        assert_eq!(clock(None), "--:--:--");
    }
}
