//! Shared picker lifetime and background font discovery.
mod catalog;
mod session;

pub(crate) use session::{ApplicationFontPicker, font_name};

use crate::{FontPicker, font_catalog};
use slint::{ModelRc, StandardListViewItem, VecModel};
use std::rc::Rc;

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

pub(crate) fn refresh_font(picker: &FontPicker, font: slint::SharedString) {
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

pub(crate) fn discover_catalog(window: slint::Weak<FontPicker>, report: impl FnOnce(String)) {
    // Only owned metadata crosses threads. A hidden reusable picker may
    // finish discovery, but no scan can show it.
    let failure = window.clone();
    if let Err(error) = crate::background::spawn_named("font-discovery", move || {
        let families = font_catalog::installed_families();
        // The result has no recipient once its window or event loop closes.
        let _ = crate::background::to_ui(window, move |picker| {
            catalog::install(&picker, families);
        });
    }) {
        report(error.to_string());
        if let Some(picker) = failure.upgrade() {
            picker.set_ready(true);
        }
    }
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
