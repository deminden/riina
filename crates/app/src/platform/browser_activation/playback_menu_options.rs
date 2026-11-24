use riina_core::{Preferences, crop, subtitle_controls, video_controls};
use riina_ui::{CatalogRow, PlaybackMenuOptions};
use slint::{ModelRc, SharedString, VecModel};

pub(super) fn from_profile(preferences: &Preferences) -> PlaybackMenuOptions {
    use crate::windows::video_filter_window::saved_filter_rows;

    PlaybackMenuOptions {
        aspects: labels(&video_controls::ASPECT_MENU_PRESETS),
        crops: labels(&crop::MENU_LABELS),
        subtitle_encodings: model(
            subtitle_controls::SUBTITLE_ENCODING_CHOICES
                .iter()
                .map(|(title, code)| CatalogRow {
                    label: (*title).into(),
                    token: (*code).into(),
                    ..CatalogRow::default()
                })
                .collect(),
        ),
        video_filters: model(saved_filter_rows(&preferences.video_filters.saved, &[])),
        audio_filters: model(saved_filter_rows(&preferences.audio_filters.saved, &[])),
    }
}

fn labels(values: &[&str]) -> ModelRc<SharedString> {
    model(values.iter().map(|value| (*value).into()).collect())
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}
