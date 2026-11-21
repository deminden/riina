//! Project preset drafts into the filter form after releasing the mutable borrow.
use crate::{PlayerWindow, VideoFilterParameterRow, VideoFiltersWindow};
use riina_core::video_filters;
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct PresetDraft {
    index: Option<usize>,
    values: Vec<String>,
}

pub(super) fn install(app: &PlayerWindow, window: &VideoFiltersWindow, is_audio: bool) {
    let draft = Rc::new(RefCell::new(PresetDraft::default()));
    let weak_window = window.as_weak();
    let select_draft = draft.clone();
    window.on_select_preset(move |index| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        if index <= 0 {
            *select_draft.borrow_mut() = PresetDraft::default();
            window.set_preset_parameters(ModelRc::new(VecModel::from(Vec::<
                VideoFilterParameterRow,
            >::new())));
            return;
        }
        let Some(preset) = usize::try_from(index - 1).ok() else {
            return;
        };
        let values = if is_audio {
            video_filters::default_audio_parameter_values(preset)
        } else {
            video_filters::default_parameter_values(preset)
        };
        let Some(values) = values else {
            return;
        };
        let selected = PresetDraft {
            index: Some(preset),
            values,
        };
        *select_draft.borrow_mut() = selected.clone();
        update_preset_form(&window, &selected, is_audio);
    });

    let text_draft = draft.clone();
    let text_window = window.as_weak();
    window.on_update_preset_text(move |parameter, value| {
        if let Some(window) = text_window.upgrade() {
            update_value(
                &window,
                &text_draft,
                parameter as usize,
                value.to_string(),
                is_audio,
            );
        }
    });

    let number_draft = draft.clone();
    let number_window = window.as_weak();
    window.on_update_preset_number(move |parameter, value| {
        if let Some(window) = number_window.upgrade() {
            update_value(
                &window,
                &number_draft,
                parameter as usize,
                value.to_string(),
                is_audio,
            );
        }
    });

    let choice_draft = draft.clone();
    let choice_window = window.as_weak();
    window.on_update_preset_choice(move |parameter, choice| {
        let preset = choice_draft.borrow().index;
        if let (Some(window), Some(preset)) = (choice_window.upgrade(), preset)
            && let Some(spec) = preset_parameters(preset, is_audio)
                .and_then(|parameters| parameters.get(parameter as usize))
            && let Some(value) = spec.choices.get(choice as usize)
        {
            update_value(
                &window,
                &choice_draft,
                parameter as usize,
                (*value).to_owned(),
                is_audio,
            );
        }
    });

    install_lut_picker(app, window, &draft, is_audio);

    let raw_draft = draft.clone();
    let raw_window = window.as_weak();
    window.on_raw_filter_edited(move || {
        *raw_draft.borrow_mut() = PresetDraft::default();
        if let Some(window) = raw_window.upgrade() {
            window.set_preset_index(0);
            window.set_preset_parameters(ModelRc::new(VecModel::from(Vec::<
                VideoFilterParameterRow,
            >::new())));
        }
    });
}

fn install_lut_picker(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    draft: &Rc<RefCell<PresetDraft>>,
    is_audio: bool,
) {
    let browse_draft = draft.clone();
    let browse_window = window.as_weak();
    let browse_app = app.as_weak();
    window.on_browse_filter_file(move || {
        let Some(app) = browse_app.upgrade() else {
            return;
        };
        if is_audio || app.get_dialog_open() {
            return;
        }
        app.set_dialog_open(true);
        app.invoke_activity();
        let weak_app = app.as_weak();
        let weak_window = browse_window.clone();
        let selection_window = browse_window.clone();
        let selection_draft = browse_draft.clone();
        let path_result = slint::spawn_local(async move {
            let selected = async {
                let Some(window) = weak_window.upgrade() else {
                    return Ok(None);
                };
                let owner = window
                    .window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?;
                let file = rfd::AsyncFileDialog::new()
                    .set_title("Choose a 3D LUT file")
                    .set_parent(owner.as_ref())
                    .pick_file()
                    .await;
                file.map(|file| {
                    file.path()
                        .to_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "The selected path contains invalid Unicode".to_owned())
                })
                .transpose()
            }
            .await;
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            app.set_dialog_open(false);
            app.invoke_activity();
            match selected {
                Ok(Some(path)) => {
                    let Some(window) = selection_window.upgrade() else {
                        return;
                    };
                    let index = selection_draft.borrow().index;
                    if let Some(index) = index
                        && let Some(parameter) =
                            preset_parameters(index, false).and_then(|parameters| {
                                parameters
                                    .iter()
                                    .position(|parameter| parameter.name == "file")
                            })
                    {
                        update_value(&window, &selection_draft, parameter, path, false);
                    }
                }
                Ok(None) => {}
                Err(error) => app.set_status(error.into()),
            }
        });
        if let Err(error) = path_result {
            app.set_dialog_open(false);
            app.invoke_activity();
            app.set_status(error.to_string().into());
        }
    });
}

fn update_value(
    window: &VideoFiltersWindow,
    draft: &RefCell<PresetDraft>,
    parameter: usize,
    value: String,
    is_audio: bool,
) {
    let updated = {
        let mut draft = draft.borrow_mut();
        let Some(slot) = draft.values.get_mut(parameter) else {
            return;
        };
        *slot = value;
        draft.clone()
    };
    update_preset_form(window, &updated, is_audio);
}

fn preset_parameters(
    index: usize,
    is_audio: bool,
) -> Option<&'static [video_filters::ParameterSpec]> {
    if is_audio {
        video_filters::audio_preset_parameters(index)
    } else {
        video_filters::preset_parameters(index)
    }
}

fn update_preset_form(window: &VideoFiltersWindow, draft: &PresetDraft, is_audio: bool) {
    let Some(index) = draft.index else {
        window.set_preset_parameters(ModelRc::new(VecModel::from(
            Vec::<VideoFilterParameterRow>::new(),
        )));
        return;
    };
    let Some(specs) = preset_parameters(index, is_audio) else {
        return;
    };
    let rows = specs
        .iter()
        .zip(&draft.values)
        .map(|(spec, value)| {
            let kind = match spec.kind {
                video_filters::ParameterKind::Text => 0,
                video_filters::ParameterKind::Integer => 1,
                video_filters::ParameterKind::Float => 2,
                video_filters::ParameterKind::Choose => 3,
            };
            VideoFilterParameterRow {
                label: spec.label.into(),
                kind,
                value: value.as_str().into(),
                path_input: spec.name == "file",
                number_value: value.parse::<f64>().unwrap_or(spec.minimum) as f32,
                minimum: spec.minimum as f32,
                maximum: spec.maximum as f32,
                step: spec.step as f32,
                choices: ModelRc::new(VecModel::from(
                    spec.choices
                        .iter()
                        .map(|choice| (*choice).into())
                        .collect::<Vec<_>>(),
                )),
                choice_index: spec
                    .choices
                    .iter()
                    .position(|choice| *choice == value)
                    .unwrap_or(0) as i32,
            }
        })
        .collect::<Vec<_>>();
    window.set_preset_parameters(ModelRc::new(VecModel::from(rows)));
    let filter = if is_audio {
        video_filters::build_audio_preset_filter(index, &draft.values)
    } else {
        video_filters::build_preset_filter(index, &draft.values)
    };
    if let Ok(filter) = filter {
        window.set_filter_string(filter.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;

    #[test]
    fn editing_preset_text_numbers_and_choices_updates_the_form()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let filters = VideoFiltersWindow::new()?;
        install(&app, &filters, false);

        filters.invoke_select_preset(1);
        filters.invoke_update_preset_text(0, "256".into());
        assert_eq!(
            filters.get_preset_parameters().row_data(0).unwrap().value,
            "256"
        );
        assert!(filters.get_filter_string().contains("256"));

        filters.invoke_select_preset(3);
        filters.invoke_update_preset_number(1, 0.75);
        assert_eq!(
            filters
                .get_preset_parameters()
                .row_data(1)
                .unwrap()
                .number_value,
            0.75
        );
        assert!(filters.get_filter_string().contains("0.75"));

        filters.invoke_select_preset(9);
        filters.invoke_update_preset_text(0, "test.cube".into());
        filters.invoke_update_preset_choice(1, 2);
        let interpolation = filters.get_preset_parameters().row_data(1).unwrap();
        assert_eq!(interpolation.choice_index, 2);
        assert_eq!(interpolation.value, "tetrahedral");
        assert!(filters.get_filter_string().contains("tetrahedral"));
        Ok(())
    }
}
