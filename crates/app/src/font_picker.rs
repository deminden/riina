//! Shared picker lifetime and background font discovery.
use crate::{FontPicker, PlayerWindow, font_catalog};
use slint::{
    ComponentHandle, ModelRc, StandardListViewItem, VecModel, winit_030::WinitWindowAccessor,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn items(values: impl Iterator<Item = String>) -> ModelRc<StandardListViewItem> {
    Rc::new(VecModel::from(
        values
            .map(|text| {
                let mut item = StandardListViewItem::default();
                item.text = text.into();
                item
            })
            .collect::<Vec<_>>(),
    ))
    .into()
}

fn selection_for_name(
    families: &[font_catalog::FontFamily],
    visible: &[usize],
    name: &str,
) -> Option<(usize, usize, usize)> {
    visible.iter().enumerate().find_map(|(row, &family)| {
        let face = families
            .get(family)?
            .faces
            .iter()
            .position(|face| face.postscript_name == name)?;
        Some((row, family, face))
    })
}

fn preferred_face_index(faces: &[font_catalog::FontFace]) -> Option<usize> {
    faces
        .iter()
        .enumerate()
        .min_by_key(|(_, face)| {
            (
                face.style != "Normal",
                face.weight.abs_diff(400),
                face.weight,
                face.stretch.abs_diff(5),
                face.postscript_name.as_str(),
            )
        })
        .map(|(index, _)| index)
}

fn preview_face(
    picker: &FontPicker,
    family: &font_catalog::FontFamily,
    face: &font_catalog::FontFace,
) {
    // Toolkit font lookup uses family plus style; PostScript names remain the
    // libass selection identity. Feeding that identity as a family can fall back.
    picker.set_preview_font_family(family.name.clone().into());
    picker.set_preview_font_weight(i32::from(face.weight).clamp(1, 1000));
    picker.set_preview_font_italic(face.style != "Normal");
    picker.set_preview_font_stretch(preview_width_scale(family, face));
}

fn preview_width_scale(family: &font_catalog::FontFamily, face: &font_catalog::FontFace) -> f32 {
    // Slint exposes weight and slant but not font stretch. Approximate static
    // OpenType width classes only when a normal-width face gives the renderer a
    // stable base face; a family with no normal face already resolves narrowly
    // or broadly through the platform font matcher.
    if face.stretch == 5 || !family.faces.iter().any(|candidate| candidate.stretch == 5) {
        return 1.0;
    }
    match face.stretch {
        1 => 0.5,
        2 => 0.625,
        3 => 0.75,
        4 => 0.875,
        6 => 1.125,
        7 => 1.25,
        8 => 1.5,
        9 => 2.0,
        _ => 1.0,
    }
}

fn refresh_selection(picker: &FontPicker, app: &PlayerWindow) {
    let font = app.get_subtitle_font();
    picker.set_custom_name(
        if font == riina_core::subtitle_controls::DEFAULT_SUBTITLE_FONT {
            "".into()
        } else {
            font
        },
    );
    if picker.get_ready() {
        picker.invoke_search(picker.get_filter_text());
    }
}

pub fn connect(app: &PlayerWindow) {
    let current = Rc::new(RefCell::new(None::<FontPicker>));
    let owned = current.clone();
    app.on_dismiss_font_picker(move || {
        if let Some(picker) = owned.borrow_mut().take() {
            picker.set_picker_active(false);
            let _ = picker.hide();
        }
    });
    let weak = app.as_weak();
    app.on_choose_subtitle_font(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if app.get_dialog_open() {
            return;
        }
        // Reuse the auxiliary window's sample, search and discovered catalog.
        // Every invocation refreshes playback selection, even if already visible.
        let existing = current.borrow().as_ref().map(ComponentHandle::clone_strong);
        if let Some(picker) = existing {
            refresh_selection(&picker, &app);
            picker.set_picker_active(true);
            if let Err(error) = picker.show() {
                picker.set_picker_active(false);
                app.set_status(error.to_string().into());
                return;
            }
            let window = picker.as_weak();
            let _ = slint::spawn_local(async move {
                if let Some(picker) = window.upgrade()
                    && let Ok(window) = picker.window().winit_window().await
                    && picker.get_picker_active()
                {
                    window.focus_window();
                }
            });
            return;
        }
        let picker = match FontPicker::new() {
            Ok(picker) => picker,
            Err(error) => {
                app.set_status(error.to_string().into());
                return;
            }
        };
        refresh_selection(&picker, &app);
        let parent = app.as_weak();
        let window = picker.as_weak();
        picker.on_accept_font(move |text| {
            let Some(app) = parent.upgrade() else {
                return;
            };
            let selected = if text.is_empty() {
                riina_core::subtitle_controls::DEFAULT_SUBTITLE_FONT.to_owned()
            } else {
                text.to_string()
            };
            app.invoke_subtitle_font_selected(selected.into());
            if let Some(window) = window.upgrade() {
                window.set_picker_active(false);
                let _ = window.hide();
            }
            app.invoke_activity();
        });
        let parent = app.as_weak();
        let window = picker.as_weak();
        picker.on_cancel(move || {
            if let Some(window) = window.upgrade() {
                window.set_picker_active(false);
                let _ = window.hide();
            }
            if let Some(app) = parent.upgrade() {
                app.invoke_activity();
            }
        });
        let parent = app.as_weak();
        let window = picker.as_weak();
        picker.window().on_close_requested(move || {
            if let Some(window) = window.upgrade() {
                window.set_picker_active(false);
            }
            if let Some(app) = parent.upgrade() {
                app.invoke_activity();
            }
            slint::CloseRequestResponse::HideWindow
        });
        if let Err(error) = picker.show() {
            app.set_status(error.to_string().into());
            return;
        }
        crate::platform::center_auxiliary_window(&picker, &app);
        let window = picker.as_weak();
        *current.borrow_mut() = Some(picker);
        // Only owned metadata crosses threads. A hidden reusable picker may
        // finish discovery, but no scan shows it; player shutdown drops it.
        let failure = window.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("font-discovery".into())
            .spawn(move || {
                let families = font_catalog::installed_families();
                let _ = window.upgrade_in_event_loop(move |picker| {
                    let families = Rc::new(families);
                    let filtered = Rc::new(RefCell::new(Vec::<usize>::new()));
                    let selected = Rc::new(Cell::new(None::<usize>));
                    let data = families.clone();
                    let map = filtered.clone();
                    let selection = selected.clone();
                    let weak = picker.as_weak();
                    picker.on_search(move |text| {
                        let Some(picker) = weak.upgrade() else {
                            return;
                        };
                        picker.invoke_cancel_reveal();
                        let indices: Vec<_> = data
                            .iter()
                            .enumerate()
                            .filter(|(_, family)| family.matches(&text))
                            .map(|(index, _)| index)
                            .collect();
                        // Clear row identity before replacing the filtered model;
                        // the same numeric row may now refer to another family.
                        picker.set_family_index(-1);
                        picker.set_face_index(-1);
                        selection.set(None);
                        picker.set_preview_font_family("".into());
                        picker.set_preview_font_weight(400);
                        picker.set_preview_font_italic(false);
                        picker.set_preview_font_stretch(1.0);
                        let rows = items(indices.iter().map(|index| data[*index].name.clone()));
                        *map.borrow_mut() = indices;
                        picker.set_faces(items(std::iter::empty()));
                        picker.set_families(rows);
                        // Restore a visible exact face without firing user intent
                        // or changing the filter/custom entry to force a match.
                        if let Some((row, family, face)) =
                            selection_for_name(&data, &map.borrow(), &picker.get_custom_name())
                        {
                            selection.set(Some(family));
                            picker.set_faces(items(
                                data[family].faces.iter().map(|face| face.label.clone()),
                            ));
                            picker.invoke_restore_selection(row as i32, face as i32);
                            preview_face(&picker, &data[family], &data[family].faces[face]);
                        }
                    });
                    let data = families.clone();
                    let map = filtered.clone();
                    let selection = selected.clone();
                    let weak = picker.as_weak();
                    picker.on_family_selected(move |row| {
                        let Some(index) = usize::try_from(row)
                            .ok()
                            .and_then(|row| map.borrow().get(row).copied())
                        else {
                            return;
                        };
                        let Some(picker) = weak.upgrade() else {
                            return;
                        };
                        selection.set(Some(index));
                        picker.set_face_index(-1);
                        picker.set_faces(items(
                            data[index].faces.iter().map(|face| face.label.clone()),
                        ));
                        if let Some(face_index) = preferred_face_index(&data[index].faces)
                            && let Some(face) = data[index].faces.get(face_index)
                        {
                            picker.invoke_restore_selection(
                                picker.get_family_index(),
                                face_index as i32,
                            );
                            picker.set_custom_name(face.postscript_name.clone().into());
                            preview_face(&picker, &data[index], face);
                        }
                    });
                    let data = families;
                    let weak = picker.as_weak();
                    picker.on_face_selected(move |row| {
                        let Some(family) = selected.get().and_then(|index| data.get(index)) else {
                            return;
                        };
                        let Some(face) = usize::try_from(row)
                            .ok()
                            .and_then(|row| family.faces.get(row))
                        else {
                            return;
                        };
                        if let Some(picker) = weak.upgrade() {
                            picker.invoke_cancel_reveal();
                            picker.set_custom_name(face.postscript_name.clone().into());
                            preview_face(&picker, family, face);
                        }
                    });
                    picker.invoke_search(picker.get_filter_text());
                    picker.set_ready(true);
                });
            })
        {
            app.set_status(error.to_string().into());
            if let Some(picker) = failure.upgrade() {
                picker.set_ready(true);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_face_selection_respects_filtered_identity_and_exact_names() {
        let family = |name: &str, faces: &[&str]| font_catalog::FontFamily {
            name: name.into(),
            aliases: vec![],
            faces: faces
                .iter()
                .map(|name| font_catalog::FontFace {
                    weight: 400,
                    stretch: 5,
                    style: "Normal".into(),
                    postscript_name: (*name).into(),
                    label: (*name).into(),
                })
                .collect(),
        };
        let data = [
            family("Alpha", &["Alpha-Regular", "Alpha-Bold"]),
            family("Beta", &["字体-常规"]),
        ];
        assert_eq!(
            selection_for_name(&data, &[1, 0], "Alpha-Bold"),
            Some((1, 0, 1))
        );
        assert_eq!(selection_for_name(&data, &[1], "Alpha-Bold"), None);
        assert_eq!(
            selection_for_name(&data, &[1], "字体-常规"),
            Some((0, 1, 0))
        );
        assert_eq!(selection_for_name(&data, &[0], "alpha-bold"), None);
        assert_eq!(selection_for_name(&data, &[0], "Alpha"), None);
        assert_eq!(selection_for_name(&data, &[0, 1], ""), None);
        assert_eq!(selection_for_name(&data, &[0, 1], "sans-serif"), None);
        assert_eq!(
            selection_for_name(&data, &[99, 0], "Alpha-Regular"),
            Some((1, 0, 0))
        );
    }

    #[test]
    fn family_selection_prefers_upright_regular_face() {
        let face = |weight, style: &str, name: &str| font_catalog::FontFace {
            weight,
            stretch: 5,
            style: style.into(),
            postscript_name: name.into(),
            label: name.into(),
        };
        let faces = [
            face(100, "Normal", "Sample-Thin"),
            face(700, "Normal", "Sample-Bold"),
            face(400, "Italic", "Sample-Italic"),
            face(400, "Normal", "Sample-Regular"),
        ];

        assert_eq!(preferred_face_index(&faces), Some(3));
    }

    #[test]
    fn family_selection_falls_back_to_nearest_upright_face() {
        let face = |weight, style: &str, name: &str| font_catalog::FontFace {
            weight,
            stretch: 5,
            style: style.into(),
            postscript_name: name.into(),
            label: name.into(),
        };
        let faces = [
            face(400, "Italic", "Sample-Italic"),
            face(700, "Normal", "Sample-Bold"),
            face(300, "Normal", "Sample-Light"),
            face(500, "Normal", "Sample-Medium"),
        ];

        assert_eq!(preferred_face_index(&faces), Some(2));
    }

    #[test]
    fn family_selection_prefers_normal_width_when_weight_and_slant_match() {
        let face = |stretch, name: &str| font_catalog::FontFace {
            weight: 400,
            stretch,
            style: "Normal".into(),
            postscript_name: name.into(),
            label: name.into(),
        };
        let faces = [face(3, "Sample-Condensed"), face(5, "Sample-Regular")];
        assert_eq!(preferred_face_index(&faces), Some(1));
    }

    #[test]
    fn preview_stretches_static_width_variants_only_with_a_normal_base_face() {
        let face = |stretch| font_catalog::FontFace {
            weight: 400,
            stretch,
            style: "Normal".into(),
            postscript_name: format!("Sample-{stretch}"),
            label: format!("Width {stretch}"),
        };
        let family = |faces: Vec<font_catalog::FontFace>| font_catalog::FontFamily {
            name: "Sample".into(),
            aliases: vec![],
            faces,
        };
        let widths = [0.5, 0.625, 0.75, 0.875, 1.0, 1.125, 1.25, 1.5, 2.0];
        let faces: Vec<_> = (1..=9).map(face).collect();
        let family_with_normal = family(faces.clone());
        for (index, selected) in faces.iter().enumerate() {
            assert_eq!(
                preview_width_scale(&family_with_normal, selected),
                widths[index]
            );
        }

        let condensed_only = family(vec![face(3)]);
        assert_eq!(
            preview_width_scale(&condensed_only, &condensed_only.faces[0]),
            1.0
        );
    }

    #[test]
    fn family_without_faces_has_no_default_selection() {
        assert_eq!(preferred_face_index(&[]), None);
    }
}
