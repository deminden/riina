//! Locale-sensitive numeric entry and display for small player fields.

use icu_decimal::DecimalFormatter;
use icu_decimal::input::{Decimal, FloatPrecision};
use icu_decimal::options::DecimalFormatterOptions;
use icu_locale::Locale;

struct NumberLocale {
    formatter: Option<DecimalFormatter>,
    decimal: char,
    grouping: Option<char>,
    digits: [char; 10],
}

impl NumberLocale {
    fn from_identifier(identifier: Option<&str>) -> Self {
        let formatter = identifier
            .and_then(|identifier| identifier.parse::<Locale>().ok())
            .and_then(|locale| {
                DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default()).ok()
            });
        let (decimal, grouping) = formatter
            .as_ref()
            .and_then(|formatter| {
                let value =
                    Decimal::try_from_f64(1_234_567.89, FloatPrecision::Magnitude(-2)).ok()?;
                let formatted = formatter.format_to_string(&value);
                let characters = formatted.chars().collect::<Vec<_>>();
                let separators = characters
                    .iter()
                    .enumerate()
                    .filter_map(|(index, character)| {
                        (index > 0
                            && index + 1 < characters.len()
                            && characters[index - 1].is_numeric()
                            && characters[index + 1].is_numeric()
                            && !character.is_numeric()
                            && !matches!(character, '+' | '-'))
                        .then_some(*character)
                    })
                    .collect::<Vec<_>>();
                let decimal = separators.last().copied()?;
                let grouping = separators
                    .iter()
                    .copied()
                    .filter(|separator| *separator != decimal)
                    .max_by_key(|separator| {
                        separators
                            .iter()
                            .filter(|candidate| **candidate == *separator)
                            .count()
                    });
                Some((decimal, grouping))
            })
            .unwrap_or(('.', Some(',')));
        let digits = formatter
            .as_ref()
            .and_then(|formatter| {
                let decimal = Decimal::try_from_str("0123456789").ok()?;
                let digits = formatter
                    .format_to_string(&decimal)
                    .chars()
                    .filter(|character| character.is_numeric())
                    .collect::<Vec<_>>();
                <[char; 10]>::try_from(digits).ok()
            })
            .unwrap_or(ASCII_DIGITS);
        Self {
            formatter,
            decimal,
            grouping,
            digits,
        }
    }

    fn format(&self, value: f64) -> String {
        if value.is_finite()
            && let Some(formatter) = &self.formatter
            && let Ok(mut decimal) =
                Decimal::try_from_f64(value, FloatPrecision::SignificantDigits(3))
        {
            decimal.trim_end();
            return formatter.format_to_string(&decimal);
        }
        fallback_format(value, self.decimal)
    }

    fn normalize_entry(&self, value: &str) -> Result<String, &'static str> {
        normalize_with_symbols(value, self.decimal, self.grouping, &self.digits)
    }
}

const ASCII_DIGITS: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

thread_local! {
    static NUMBER_LOCALE: std::cell::RefCell<Option<(Option<String>, NumberLocale)>> = const { std::cell::RefCell::new(None) };
}

fn with_locale<T>(action: impl FnOnce(&NumberLocale) -> T) -> T {
    let identifier = crate::platform::language::current_locale_identifier();
    NUMBER_LOCALE.with(|locale| with_cached_locale(locale, identifier, action))
}

fn with_cached_locale<T>(
    cache: &std::cell::RefCell<Option<(Option<String>, NumberLocale)>>,
    identifier: Option<String>,
    action: impl FnOnce(&NumberLocale) -> T,
) -> T {
    let mut cache = cache.borrow_mut();
    if cache
        .as_ref()
        .is_none_or(|(cached_identifier, _)| cached_identifier != &identifier)
    {
        *cache = Some((
            identifier.clone(),
            NumberLocale::from_identifier(identifier.as_deref()),
        ));
    }
    action(&cache.as_ref().expect("locale cache was initialized").1)
}

pub(crate) fn format_delay(value: f64) -> String {
    with_locale(|locale| locale.format(value))
}

pub(crate) fn normalize_delay_entry(value: &str) -> Result<String, &'static str> {
    with_locale(|locale| locale.normalize_entry(value))
}

fn normalize_with_symbols(
    value: &str,
    decimal: char,
    grouping: Option<char>,
    digits: &[char; 10],
) -> Result<String, &'static str> {
    let localized = value
        .chars()
        .filter(|character| !matches!(*character, '\u{061c}' | '\u{200e}' | '\u{200f}'))
        .map(|character| {
            if character.is_ascii_digit() {
                character
            } else {
                digits
                    .iter()
                    .position(|digit| *digit == character)
                    .map_or(character, |value| char::from(b'0' + value as u8))
            }
        })
        .collect::<String>();
    let value = localized.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let (mantissa, exponent) = value.find(['e', 'E']).map_or((value, None), |index| {
        (&value[..index], Some(&value[index..]))
    });
    let mut pieces = mantissa.split(decimal);
    let integer = pieces.next().unwrap_or_default();
    let fraction = pieces.next();
    if pieces.next().is_some()
        || fraction.is_some_and(|value| value.contains(grouping.unwrap_or('\0')))
    {
        return Err("Enter a numeric delay in seconds");
    }
    let sign = integer.starts_with(['-', '+']);
    let unsigned = if sign { &integer[1..] } else { integer };
    let digits = if let Some(grouping) = grouping {
        let groups = unsigned.split(grouping).collect::<Vec<_>>();
        if groups.len() > 1
            && (groups[0].is_empty()
                || groups[0].len() > 3
                || groups.iter().skip(1).any(|group| group.len() != 3))
        {
            return Err("Enter a numeric delay in seconds");
        }
        groups.concat()
    } else {
        unsigned.to_owned()
    };
    if digits.chars().any(|character| !character.is_ascii_digit())
        || fraction.is_some_and(|value| value.chars().any(|character| !character.is_ascii_digit()))
    {
        return Err("Enter a numeric delay in seconds");
    }
    let mut normalized = String::with_capacity(value.len());
    if sign {
        normalized.push(integer.chars().next().unwrap_or_default());
    }
    normalized.push_str(&digits);
    if let Some(fraction) = fraction {
        normalized.push('.');
        normalized.push_str(fraction);
    }
    if let Some(exponent) = exponent {
        if exponent.len() < 2
            || !exponent[1..]
                .trim_start_matches(['+', '-'])
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return Err("Enter a numeric delay in seconds");
        }
        normalized.push_str(exponent);
    }
    Ok(normalized)
}

fn fallback_format(value: f64, decimal: char) -> String {
    let mut formatted = format!("{value:.6}");
    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    if decimal != '.' {
        formatted = formatted.replace('.', &decimal.to_string());
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::{
        ASCII_DIGITS, NumberLocale, fallback_format, normalize_with_symbols, with_cached_locale,
    };
    use std::cell::RefCell;

    #[test]
    fn icu_delay_formatting_matches_locale_decimal_and_grouping() {
        let german = NumberLocale::from_identifier(Some("de-DE"));
        assert_eq!(german.format(1_234.5), "1.230");
        assert_eq!(german.normalize_entry("1.234,5"), Ok("1234.5".into()));
        assert_eq!(german.format(12.3456), "12,3");
        assert_eq!(german.format(1.0), "1");
    }

    #[test]
    fn localized_entry_parses_decimal_and_grouping_symbols() {
        assert_eq!(
            normalize_with_symbols("1.234,5", ',', Some('.'), &ASCII_DIGITS),
            Ok("1234.5".into())
        );
        assert_eq!(
            normalize_with_symbols("-12,345678", ',', Some('.'), &ASCII_DIGITS),
            Ok("-12.345678".into())
        );
        assert_eq!(
            normalize_with_symbols("1,234,567.89", '.', Some(','), &ASCII_DIGITS),
            Ok("1234567.89".into())
        );
    }

    #[test]
    fn localized_entry_rejects_malformed_grouping_and_extra_decimal_marks() {
        for value in ["12.34,5", "1,2,3", "1.2.3", "1,2.3"] {
            assert!(
                normalize_with_symbols(value, ',', Some('.'), &ASCII_DIGITS).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn fallback_delay_format_uses_locale_decimal_and_omits_zeroes() {
        assert_eq!(fallback_format(1.25, ','), "1,25");
        assert_eq!(fallback_format(1.0, ','), "1");
    }

    #[test]
    fn native_digits_and_directional_marks_round_trip_with_icu_locale_data() {
        let arabic = NumberLocale::from_identifier(Some("ar-EG"));
        let displayed = arabic.format(1_234.5);
        assert_eq!(arabic.normalize_entry(&displayed), Ok("1230".into()));
        let displayed_negative = arabic.format(-12.345);
        assert_eq!(
            arabic.normalize_entry(&displayed_negative),
            Ok("-12.3".into())
        );
    }

    #[test]
    fn cached_formatter_refreshes_when_the_system_locale_changes() {
        let cache = RefCell::new(None);
        let german = with_cached_locale(&cache, Some("de-DE".into()), |locale| locale.format(1.25));
        let english =
            with_cached_locale(&cache, Some("en-US".into()), |locale| locale.format(1.25));
        assert_eq!(german, "1,25");
        assert_eq!(english, "1.25");
    }
}
