//! Track language badges are presentation, never replacements for mpv metadata.

#[cfg(any(not(target_os = "macos"), test))]
use icu_experimental::displaynames::{
    DisplayNamesOptions, LanguageDisplay, multi::LocaleDisplayNamesFormatter,
};
#[cfg(any(not(target_os = "macos"), test))]
use icu_locale::{Locale, LocaleCanonicalizer};

pub(crate) struct LanguageNames {
    #[cfg(not(target_os = "macos"))]
    names: UnicodeNames,
}

impl LanguageNames {
    pub(crate) fn current() -> Self {
        Self {
            #[cfg(not(target_os = "macos"))]
            names: UnicodeNames::new(
                crate::platform::language::current_locale_identifier().as_deref(),
            ),
        }
    }

    pub(crate) fn display_name(&self, identifier: Option<&str>) -> String {
        let Some(identifier) = identifier.filter(|identifier| !identifier.is_empty()) else {
            return String::new();
        };
        #[cfg(target_os = "macos")]
        {
            crate::platform::language::language_name(identifier).unwrap_or_default()
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.names.display_name(identifier)
        }
    }
}

#[cfg(any(not(target_os = "macos"), test))]
struct UnicodeNames {
    formatter: Option<LocaleDisplayNamesFormatter>,
    canonicalizer: LocaleCanonicalizer,
}

#[cfg(any(not(target_os = "macos"), test))]
impl UnicodeNames {
    fn new(display_locale: Option<&str>) -> Self {
        let canonicalizer = LocaleCanonicalizer::new_extended();
        let mut options = DisplayNamesOptions::default();
        // Preserve separate language/script/region names in locale identifiers.
        // Dialect labels would turn en-US into "American English" instead.
        options.language_display = LanguageDisplay::Standard;
        let formatter = display_locale
            .and_then(parse_identifier)
            .and_then(|mut locale| {
                canonicalizer.canonicalize(&mut locale);
                LocaleDisplayNamesFormatter::try_new(locale.into(), options).ok()
            })
            .or_else(|| {
                LocaleDisplayNamesFormatter::try_new(icu_locale::locale!("en").into(), options).ok()
            });
        Self {
            formatter,
            canonicalizer,
        }
    }

    fn display_name(&self, identifier: &str) -> String {
        let Some(mut locale) = parse_identifier(identifier) else {
            return String::new();
        };
        // Media commonly supplies ISO 639-2 bibliographic codes (ger/fre/chi).
        // CLDR aliases cover these without a partial hand-maintained dictionary.
        self.canonicalizer.canonicalize(&mut locale);
        self.formatter.as_ref().map_or_else(
            || identifier.to_owned(),
            |formatter| formatter.of(&locale).into_owned(),
        )
    }
}

#[cfg(any(not(target_os = "macos"), test))]
fn parse_identifier(identifier: &str) -> Option<Locale> {
    // Foundation accepts locale-style underscores as well as BCP 47 hyphens.
    // Keep script and region subtags; truncating at '-' loses distinctions such
    // as Traditional Chinese and Brazilian Portuguese in the track selector.
    identifier.replace('_', "-").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_language_names_cover_iso_aliases_and_locale_components() {
        let names = UnicodeNames::new(Some("en-IE"));
        for (code, expected) in [
            ("eng", "English"),
            ("ger", "German"),
            ("deu", "German"),
            ("fre", "French"),
            ("fra", "French"),
            ("chi", "Chinese"),
            ("zho", "Chinese"),
            ("jpn", "Japanese"),
            ("en-US", "English (United States)"),
            ("pt_BR", "Portuguese (Brazil)"),
            ("zh-Hant", "Chinese (Traditional)"),
            ("sr-Latn-RS", "Serbian (Latin, Serbia)"),
        ] {
            assert_eq!(names.display_name(code), expected, "{code}");
        }
    }

    #[test]
    fn language_badges_follow_the_display_locale() {
        assert_eq!(
            UnicodeNames::new(Some("de-DE")).display_name("eng"),
            "Englisch"
        );
        assert_eq!(
            UnicodeNames::new(Some("fr-FR")).display_name("deu"),
            "allemand"
        );
        assert_eq!(UnicodeNames::new(Some("ja-JP")).display_name("eng"), "英語");
        assert_eq!(UnicodeNames::new(None).display_name("eng"), "English");
    }

    #[test]
    fn unspecified_and_unknown_languages_do_not_invent_a_track_language() {
        let names = UnicodeNames::new(Some("en"));
        // The sidebar requests a name for und too; only menu labels suppress it.
        assert_eq!(names.display_name("und"), "Unknown language");
        assert_eq!(names.display_name("qzz"), "qzz");
        for invalid in ["", "English subtitles", "en\n", "../en", "en--US"] {
            assert_eq!(names.display_name(invalid), "", "{invalid:?}");
        }
        assert_eq!(LanguageNames::current().display_name(None), "");
        assert_eq!(LanguageNames::current().display_name(Some("")), "");
    }
}
