//! Ordered language tokens keep the settings editor and mpv CSV preferences in sync.

use crate::PreferencesWindow;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

fn parse(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn model(value: &str) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        parse(value)
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
}

fn add(value: &str, entry: &str) -> Option<String> {
    let mut tokens = parse(value);
    let mut changed = false;
    for token in entry
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if tokens
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(token))
        {
            continue;
        }
        tokens.push(token.to_lowercase());
        changed = true;
    }
    changed.then(|| tokens.join(","))
}

fn remove(value: &str, index: i32) -> Option<String> {
    let mut tokens = parse(value);
    let index = usize::try_from(index).ok()?;
    if index >= tokens.len() {
        return None;
    }
    tokens.remove(index);
    Some(tokens.join(","))
}

fn apply_subtitle(window: &PreferencesWindow, value: String) {
    window.set_subtitle_preferred_languages_text(value.clone().into());
    window.set_subtitle_language_tokens(model(&value));
}

fn apply_audio(window: &PreferencesWindow, value: String) {
    window.set_audio_preferred_languages(value.clone().into());
    window.set_audio_language_tokens(model(&value));
}

pub(crate) fn connect(window: &PreferencesWindow) {
    let weak = window.as_weak();
    window.on_add_subtitle_language_token(move |entry| {
        let Some(window) = weak.upgrade() else { return };
        let Some(value) = add(
            window.get_subtitle_preferred_languages_text().as_str(),
            entry.as_str(),
        ) else {
            return;
        };
        apply_subtitle(&window, value);
        window.invoke_settings_changed();
    });
    let weak = window.as_weak();
    window.on_remove_subtitle_language_token(move |index| {
        let Some(window) = weak.upgrade() else { return };
        let Some(value) = remove(
            window.get_subtitle_preferred_languages_text().as_str(),
            index,
        ) else {
            return;
        };
        apply_subtitle(&window, value);
        window.invoke_settings_changed();
    });
    let weak = window.as_weak();
    window.on_add_audio_language_token(move |entry| {
        let Some(window) = weak.upgrade() else { return };
        let Some(value) = add(
            window.get_audio_preferred_languages().as_str(),
            entry.as_str(),
        ) else {
            return;
        };
        apply_audio(&window, value);
        window.invoke_settings_changed();
    });
    let weak = window.as_weak();
    window.on_remove_audio_language_token(move |index| {
        let Some(window) = weak.upgrade() else { return };
        let Some(value) = remove(window.get_audio_preferred_languages().as_str(), index) else {
            return;
        };
        apply_audio(&window, value);
        window.invoke_settings_changed();
    });
}

pub(crate) fn install_subtitle(window: &PreferencesWindow, value: &str) {
    window.set_subtitle_language_tokens(model(value));
}

pub(crate) fn install_audio(window: &PreferencesWindow, value: &str) {
    window.set_audio_language_tokens(model(value));
}

#[cfg(test)]
mod tests {
    use super::{add, parse, remove};

    #[test]
    fn tokens_keep_preference_order_and_split_pasted_csv() {
        assert_eq!(add("eng", "deu, spa"), Some("eng,deu,spa".into()));
        assert_eq!(parse("eng, deu,spa"), ["eng", "deu", "spa"]);
    }

    #[test]
    fn duplicate_tokens_are_ignored_without_changing_existing_order() {
        assert_eq!(add("eng,deu", "ENG, spa"), Some("eng,deu,spa".into()));
        assert_eq!(add("eng,deu", "ENG"), None);
    }

    #[test]
    fn removing_a_token_keeps_remaining_values_and_rejects_invalid_indices() {
        assert_eq!(remove("eng, deu,spa", 1), Some("eng,spa".into()));
        assert_eq!(remove("eng,deu", -1), None);
        assert_eq!(remove("eng,deu", 2), None);
    }
}
