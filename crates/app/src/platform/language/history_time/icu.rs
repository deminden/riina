use super::{DateTime, Local, Style};
use chrono::{Datelike, Timelike};
use icu_datetime::{
    DateTimeFormatter, DateTimeFormatterPreferences, NoCalendarFormatter,
    fieldsets::{T, YMD, YMDT},
    input::{Date, DateTime as IcuDateTime, Time},
    options::{Alignment, YearStyle},
    preferences::HourCycle,
};
use icu_locale::Locale;

pub(super) struct Formatter {
    heading: Option<DateTimeFormatter<YMD>>,
    time: Option<NoCalendarFormatter<T>>,
    date_time: Option<DateTimeFormatter<YMDT>>,
}

impl Formatter {
    pub(super) fn current() -> Self {
        let mut preferences = DateTimeFormatterPreferences::from(regional_locale());
        preferences.hour_cycle = clock_override().or(preferences.hour_cycle);
        Self::new(preferences)
    }

    fn new(preferences: DateTimeFormatterPreferences) -> Self {
        Self {
            heading: DateTimeFormatter::try_new(
                preferences,
                YMD::medium().with_year_style(YearStyle::Full),
            )
            .ok(),
            time: NoCalendarFormatter::try_new(
                preferences,
                T::hm().with_alignment(Alignment::Column),
            )
            .ok(),
            date_time: DateTimeFormatter::try_new(
                preferences,
                YMD::short()
                    .with_time_hm()
                    .with_alignment(Alignment::Column),
            )
            .ok(),
        }
    }

    pub(super) fn format(&self, local: &DateTime<Local>, style: Style) -> Option<String> {
        self.format_local(local.naive_local(), style)
    }

    fn format_local(&self, local: chrono::NaiveDateTime, style: Style) -> Option<String> {
        let date = Date::try_new_iso(local.year(), local.month() as u8, local.day() as u8).ok()?;
        let time = Time::try_new(local.hour() as u8, local.minute() as u8, 0, 0).ok()?;
        Some(match style {
            Style::DayHeading => self.heading.as_ref()?.format(&date).to_string(),
            Style::Time => self.time.as_ref()?.format(&time).to_string(),
            Style::DateTime => self
                .date_time
                .as_ref()?
                .format(&IcuDateTime { date, time })
                .to_string(),
        })
    }
}

fn regional_locale() -> Locale {
    #[cfg(target_os = "linux")]
    let identifier = ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .filter_map(|setting| std::env::var(setting).ok())
        .find(|setting| !setting.is_empty())
        .map(|value| {
            value
                .split(['.', '@'])
                .next()
                .unwrap_or_default()
                .replace('_', "-")
        });
    #[cfg(not(target_os = "linux"))]
    let identifier = super::super::current_locale_identifier();
    identifier
        .and_then(|locale| locale.parse().ok())
        .unwrap_or(icu_locale::locale!("en"))
}

#[cfg(target_os = "windows")]
fn clock_override() -> Option<HourCycle> {
    use windows::{
        Win32::Globalization::{GetLocaleInfoEx, LOCALE_ITIME},
        core::PCWSTR,
    };
    let mut hour = [0_u16; 2];
    // SAFETY: the null locale requests the user's locale and the buffer holds one digit and NUL.
    let size = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_ITIME, Some(&mut hour)) };
    match (size, char::from_u32(u32::from(hour[0]))) {
        (2, Some('0')) => Some(HourCycle::H12),
        (2, Some('1')) => Some(HourCycle::H23),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn clock_override() -> Option<HourCycle> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_labels_follow_regional_date_order_and_the_hour_cycle() {
        let date = chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let played = date.and_hms_opt(4, 9, 53).unwrap();
        let american = Formatter::new(icu_locale::locale!("en-US").into());
        let french = Formatter::new(icu_locale::locale!("fr-FR").into());
        assert_eq!(
            american.format_local(played, Style::DayHeading).unwrap(),
            "Jan 15, 2025"
        );
        assert_eq!(
            french.format_local(played, Style::DayHeading).unwrap(),
            "15 janv. 2025"
        );
        assert_eq!(french.format_local(played, Style::Time).unwrap(), "04:09");
        assert!(
            american
                .format_local(played, Style::Time)
                .unwrap()
                .starts_with("04:09")
        );
        assert!(
            american
                .format_local(played, Style::Time)
                .unwrap()
                .ends_with("AM")
        );
        assert!(
            american
                .format_local(played, Style::DateTime)
                .unwrap()
                .starts_with("01/15/25")
        );
        assert!(
            french
                .format_local(played, Style::DateTime)
                .unwrap()
                .starts_with("15/01/2025")
        );
    }
}
