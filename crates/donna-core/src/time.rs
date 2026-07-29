use donna_config::TimeFormatMode;
use std::sync::atomic::{AtomicBool, Ordering};

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const DAY_SECONDS: i64 = 86_400;

/// Process-wide time display preference. Every tool that formats a timestamp
/// for the user goes through `format_unix_timestamp_human` deep in the
/// harness, with no config in scope at the call site — so this is set once
/// from `AppConfig` at startup (see `configure_time_format`) and read as
/// ambient state, the same way `local_offset_seconds` treats the OS
/// timezone as ambient rather than threading it through every call.
static USE_TWELVE_HOUR_CLOCK: AtomicBool = AtomicBool::new(false);

/// Sets the process-wide time format from the user's configuration. Call
/// once at startup, before any timestamp is formatted for display.
pub fn configure_time_format(mode: TimeFormatMode) {
    USE_TWELVE_HOUR_CLOCK.store(mode == TimeFormatMode::TwelveHour, Ordering::Relaxed);
}

/// Formats a Unix timestamp (seconds) as a human-readable local date and
/// time, e.g. "15:30" for something happening today, "Wed Jul 29, 15:30" for
/// another day this year, or "Wed Jul 29 2027, 15:30" for a different year.
/// This is what tools should use to show timestamps to a user, instead of a
/// raw seconds-since-epoch or an always-UTC-and-always-fully-dated string.
pub fn format_unix_timestamp_human(seconds: i64) -> String {
    let now = now_seconds();
    format_local_relative_to(seconds, local_offset_seconds(seconds), now, local_offset_seconds(now))
}

fn format_local_relative_to(seconds: i64, offset: i32, now: i64, now_offset: i32) -> String {
    let local_seconds = seconds + i64::from(offset);
    let now_local_seconds = now + i64::from(now_offset);
    let local_days = local_seconds.div_euclid(DAY_SECONDS);
    let now_local_days = now_local_seconds.div_euclid(DAY_SECONDS);

    let time = format_hh_mm(local_seconds);
    if local_days == now_local_days {
        return time;
    }

    let (year, month, day) = civil_from_days(local_days);
    let (now_year, _, _) = civil_from_days(now_local_days);
    let weekday = WEEKDAY_NAMES[((local_days + 4).rem_euclid(7)) as usize];
    let month_name = MONTH_NAMES[(month - 1) as usize];

    if year == now_year {
        format!("{weekday} {month_name} {day}, {time}")
    } else {
        format!("{weekday} {month_name} {day} {year}, {time}")
    }
}

fn format_hh_mm(local_seconds: i64) -> String {
    format_hh_mm_with_clock(local_seconds, USE_TWELVE_HOUR_CLOCK.load(Ordering::Relaxed))
}

fn format_hh_mm_with_clock(local_seconds: i64, twelve_hour: bool) -> String {
    let seconds_of_day = local_seconds.rem_euclid(DAY_SECONDS);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day / 60) % 60;

    if !twelve_hour {
        return format!("{hour:02}:{minute:02}");
    }

    let period = if hour < 12 { "AM" } else { "PM" };
    let hour_12 = match hour % 12 {
        0 => 12,
        other => other,
    };
    format!("{hour_12}:{minute:02} {period}")
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// The local UTC offset in seconds for the given Unix timestamp (positive
/// east of UTC), DST-aware via the OS's own timezone database. Falls back to
/// UTC (0) on platforms without a local-time lookup implemented.
#[cfg(unix)]
fn local_offset_seconds(unix_time: i64) -> i32 {
    // SAFETY: `tm` is a plain-old-data struct of integers; zero-initializing
    // it is valid, and `localtime_r` fully populates it from `time`.
    unsafe {
        let time = unix_time as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&time, &mut tm).is_null() {
            return 0;
        }
        tm.tm_gmtoff as i32
    }
}

#[cfg(windows)]
fn local_offset_seconds(unix_time: i64) -> i32 {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{
        FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime, TIME_ZONE_INFORMATION,
    };

    // FILETIME counts 100ns intervals since 1601-01-01; the Unix epoch
    // (1970-01-01) falls 11,644,473,600 seconds after that.
    const UNIX_TO_FILETIME_EPOCH_SECONDS: i64 = 11_644_473_600;
    let filetime_100ns = (unix_time + UNIX_TO_FILETIME_EPOCH_SECONDS) as u64 * 10_000_000;
    let file_time = FILETIME {
        dwLowDateTime: filetime_100ns as u32,
        dwHighDateTime: (filetime_100ns >> 32) as u32,
    };

    // SAFETY: `file_time` is a fully populated FILETIME; the SYSTEMTIME
    // outputs are plain integer structs safe to zero-initialize, and each
    // Win32 call fully populates its output on success (checked below).
    unsafe {
        let mut utc_time: SYSTEMTIME = std::mem::zeroed();
        if FileTimeToSystemTime(&file_time, &mut utc_time) == 0 {
            return 0;
        }

        let mut local_time: SYSTEMTIME = std::mem::zeroed();
        // A null time zone pointer tells Windows to use the currently
        // active system time zone's rules for this specific UTC moment,
        // which is DST-aware.
        if SystemTimeToTzSpecificLocalTime(
            std::ptr::null::<TIME_ZONE_INFORMATION>(),
            &utc_time,
            &mut local_time,
        ) == 0
        {
            return 0;
        }

        let local_epoch_as_utc = days_from_civil(
            local_time.wYear as i64,
            u32::from(local_time.wMonth),
            u32::from(local_time.wDay),
        ) * DAY_SECONDS
            + i64::from(local_time.wHour) * 3_600
            + i64::from(local_time.wMinute) * 60
            + i64::from(local_time.wSecond);

        (local_epoch_as_utc - unix_time) as i32
    }
}

#[cfg(not(any(unix, windows)))]
fn local_offset_seconds(_unix_time: i64) -> i32 {
    0
}

/// Howard Hinnant's civil-from-days algorithm: converts a day count since the
/// Unix epoch into a proleptic Gregorian (year, month, day).
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u8, day as u8)
}

/// Howard Hinnant's days-from-civil algorithm: the inverse of
/// [`civil_from_days`], converting a proleptic Gregorian (year, month, day)
/// into a day count since the Unix epoch. Only used on Windows, to turn the
/// `SYSTEMTIME` Win32 gives back into a comparable epoch value.
#[cfg(windows)]
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_of_year = if month > 2 { month - 3 } else { month + 9 } as i64;
    let day_of_year = (153 * month_of_year + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::{format_hh_mm_with_clock, format_local_relative_to, local_offset_seconds};

    #[test]
    fn twelve_hour_clock_formats_morning_noon_midnight_and_evening() {
        assert_eq!(format_hh_mm_with_clock(0, true), "12:00 AM");
        assert_eq!(format_hh_mm_with_clock(9 * 3_600 + 5 * 60, true), "9:05 AM");
        assert_eq!(format_hh_mm_with_clock(12 * 3_600, true), "12:00 PM");
        assert_eq!(
            format_hh_mm_with_clock(15 * 3_600 + 30 * 60, true),
            "3:30 PM"
        );
        assert_eq!(
            format_hh_mm_with_clock(23 * 3_600 + 59 * 60, true),
            "11:59 PM"
        );
    }

    #[test]
    fn twenty_four_hour_clock_is_unchanged_by_the_twelve_hour_path() {
        assert_eq!(
            format_hh_mm_with_clock(15 * 3_600 + 30 * 60, false),
            "15:30"
        );
    }

    #[test]
    fn same_local_day_shows_only_the_time() {
        // now = 2026-07-27 09:00 UTC (Monday). Event at 15:30 UTC, same day.
        let now = 1_785_142_800; // 2026-07-27T09:00:00Z
        let event = 1_785_166_200; // 2026-07-27T15:30:00Z

        assert_eq!(format_local_relative_to(event, 0, now, 0), "15:30");
    }

    #[test]
    fn different_day_same_year_shows_weekday_month_and_day() {
        let now = 1_785_142_800; // 2026-07-27T09:00:00Z (Monday)
        let event = 1_785_339_000; // 2026-07-29T15:30:00Z (Wednesday)

        assert_eq!(
            format_local_relative_to(event, 0, now, 0),
            "Wed Jul 29, 15:30"
        );
    }

    #[test]
    fn different_year_also_shows_the_year() {
        let now = 1_785_142_800; // 2026-07-27T09:00:00Z
        let event = 1_835_490_600; // 2028-03-01T02:30:00Z (Wednesday)

        assert_eq!(
            format_local_relative_to(event, 0, now, 0),
            "Wed Mar 1 2028, 02:30"
        );
    }

    #[test]
    fn a_positive_offset_can_push_an_event_into_the_next_local_day() {
        // 2026-07-27T23:30:00Z is still "today" in UTC, but a UTC+2 local
        // offset makes it 2026-07-28T01:30 locally — a different day.
        let now = 1_785_142_800; // 2026-07-27T09:00:00Z
        let event = 1_785_195_000; // 2026-07-27T23:30:00Z
        let offset = 2 * 3_600;

        assert_eq!(
            format_local_relative_to(event, offset, now, offset),
            "Tue Jul 28, 01:30"
        );
    }

    #[test]
    fn local_offset_seconds_is_a_plausible_utc_offset() {
        let offset = local_offset_seconds(1_785_142_800);
        assert!((-12 * 3_600..=14 * 3_600).contains(&offset));
    }

    #[cfg(windows)]
    #[test]
    fn days_from_civil_is_the_inverse_of_civil_from_days() {
        use super::{civil_from_days, days_from_civil};

        for days in [-719_468_i64, -1, 0, 1, 18_000, 100_000] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(i64::from(year), u32::from(month), u32::from(day)), days);
        }
    }
}
