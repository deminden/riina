use super::{DateTime, Local, Style};
use objc2::rc::Retained;
use objc2_foundation::{NSDate, NSDateFormatter, NSDateFormatterStyle, NSString};

pub(super) struct Formatter {
    heading: Retained<NSDateFormatter>,
    time: Retained<NSDateFormatter>,
    date_time: Retained<NSDateFormatter>,
}

impl Formatter {
    pub(super) fn current() -> Self {
        objc2::rc::autoreleasepool(|_| {
            let heading = NSDateFormatter::new();
            heading.setDateStyle(NSDateFormatterStyle::MediumStyle);
            heading.setTimeStyle(NSDateFormatterStyle::NoStyle);
            Self {
                heading,
                time: with_padded_hour("jmm"),
                date_time: with_padded_hour("yyMMddjmm"),
            }
        })
    }

    pub(super) fn format(&self, local: &DateTime<Local>, style: Style) -> Option<String> {
        objc2::rc::autoreleasepool(|_| {
            let instant = NSDate::dateWithTimeIntervalSince1970(
                local.timestamp() as f64 + f64::from(local.timestamp_subsec_millis()) / 1000.0,
            );
            let formatter = match style {
                Style::DayHeading => &self.heading,
                Style::Time => &self.time,
                Style::DateTime => &self.date_time,
            };
            let formatted = formatter.stringFromDate(&instant).to_string();
            (!formatted.is_empty()).then_some(formatted)
        })
    }
}

fn with_padded_hour(template: &str) -> Retained<NSDateFormatter> {
    let formatter = NSDateFormatter::new();
    formatter.setLocalizedDateFormatFromTemplate(&NSString::from_str(template));
    let format = formatter.dateFormat().to_string();
    let padded = pad_hour(&format);
    if padded != format {
        formatter.setDateFormat(Some(&NSString::from_str(&padded)));
    }
    formatter
}

fn pad_hour(format: &str) -> String {
    let mut output = String::with_capacity(format.len() + 1);
    let mut quoted = false;
    let mut previous = None;
    let mut fields = format.chars().peekable();
    while let Some(field) = fields.next() {
        output.push(field);
        if field == '\'' {
            quoted = !quoted;
        } else if !quoted
            && matches!(field, 'h' | 'H' | 'k' | 'K')
            && previous != Some(field)
            && fields.peek() != Some(&field)
        {
            output.push(field);
        }
        previous = Some(field);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hour_padding_preserves_quoted_text_and_already_padded_hours() {
        assert_eq!(pad_hour("d MMM 'at' h:mm a"), "d MMM 'at' hh:mm a");
        assert_eq!(pad_hour("HH:mm 'h'"), "HH:mm 'h'");
        assert_eq!(pad_hour("K:mm ''a"), "KK:mm ''a");
    }
}
