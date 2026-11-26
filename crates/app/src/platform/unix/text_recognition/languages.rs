use icu_locale::{Locale, LocaleCanonicalizer, LocaleExpander};
use std::{
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) struct Configuration {
    pub(super) directory: Option<CString>,
    pub(super) languages: CString,
}

pub(super) fn configuration() -> Configuration {
    let locale = gtk::glib::language_names();
    let configured = std::env::var_os("TESSDATA_PREFIX").filter(|path| !path.is_empty());
    let directories = if let Some(path) = configured {
        vec![PathBuf::from(path)]
    } else {
        gtk::glib::system_data_dirs()
            .into_iter()
            .flat_map(|root| {
                [
                    root.join("tesseract-ocr/5/tessdata"),
                    root.join("tessdata"),
                    root.join("tesseract/tessdata"),
                ]
            })
            .collect()
    };
    for directory in directories {
        let mut models = installed_models(&directory);
        if !models.is_empty()
            && let Ok(path) = CString::new(directory.as_os_str().as_bytes())
        {
            models.sort_unstable();
            let selected = preferred_models(&models, locale.iter().map(|name| name.as_str()));
            return Configuration {
                directory: Some(path),
                languages: selected,
            };
        }
    }
    // The library also knows its build-time data directory on custom installations.
    Configuration {
        directory: None,
        languages: c"eng".to_owned(),
    }
}

fn installed_models(directory: &Path) -> Vec<String> {
    let Ok(files) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    files
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "traineddata" || !path.is_file() {
                return None;
            }
            let model = path.file_stem()?.to_str()?;
            (model != "osd"
                && model
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
            .then(|| model.into())
        })
        .collect()
}

fn preferred_models<'a>(models: &[String], locale_names: impl Iterator<Item = &'a str>) -> CString {
    let canonicalizer = LocaleCanonicalizer::new_extended();
    let expander = LocaleExpander::new_extended();
    let preferred = locale_names
        .filter_map(|name| {
            let raw = name.split(['.', '@']).next()?.replace('_', "-");
            let mut locale: Locale = raw.parse().ok()?;
            canonicalizer.canonicalize(&mut locale);
            expander.maximize(&mut locale.id);
            Some(locale)
        })
        .find_map(|locale| {
            models.iter().find(|model| match model.as_str() {
                "chi_sim" => {
                    locale.id.language.as_str() == "zh"
                        && locale
                            .id
                            .script
                            .is_some_and(|script| script.as_str() == "Hans")
                }
                "chi_tra" => {
                    locale.id.language.as_str() == "zh"
                        && locale
                            .id
                            .script
                            .is_some_and(|script| script.as_str() == "Hant")
                }
                model => model.parse::<Locale>().ok().is_some_and(|mut candidate| {
                    canonicalizer.canonicalize(&mut candidate);
                    candidate.id.language == locale.id.language
                }),
            })
        });
    let english = models.iter().any(|model| model == "eng");
    let selected = match preferred {
        Some(model) if model != "eng" && english => format!("{model}+eng"),
        Some(model) => model.clone(),
        None if english => "eng".into(),
        None => models.first().cloned().unwrap_or_else(|| "eng".into()),
    };
    CString::new(selected).unwrap_or_else(|_| c"eng".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_installed_locale_and_falls_back_when_only_one_model_exists() {
        let models = ["chi_sim", "chi_tra", "deu", "eng", "jpn"].map(str::to_owned);
        assert_eq!(
            preferred_models(&models, ["de_DE.UTF-8"].into_iter()),
            c"deu+eng".to_owned()
        );
        assert_eq!(
            preferred_models(&models, ["zh_TW"].into_iter()),
            c"chi_tra+eng".to_owned()
        );
        assert_eq!(
            preferred_models(&models, ["zh_CN"].into_iter()),
            c"chi_sim+eng".to_owned()
        );
        assert_eq!(
            preferred_models(&models, ["C"].into_iter()),
            c"eng".to_owned()
        );
        assert_eq!(
            preferred_models(&["jpn".into()], ["C"].into_iter()),
            c"jpn".to_owned()
        );
    }
}
