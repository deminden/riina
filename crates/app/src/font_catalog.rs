//! Installed-font metadata for the shared picker; no font bytes leave discovery.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontFace {
    pub weight: u16,
    /// OpenType OS/2 width class, from 1 (ultra-condensed) to 9 (ultra-expanded).
    pub stretch: u8,
    pub style: String,
    pub postscript_name: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily {
    pub name: String,
    pub aliases: Vec<String>,
    pub faces: Vec<FontFace>,
}

impl FontFamily {
    pub fn matches(&self, search: &str) -> bool {
        let search = search.to_lowercase();
        self.name.to_lowercase().contains(&search)
            || self
                .aliases
                .iter()
                .any(|name| name.to_lowercase().contains(&search))
    }
}

pub fn installed_families() -> Vec<FontFamily> {
    let mut database = fontdb::Database::new();
    // fontdb discovers system/user font directories on each desktop target.
    // The picker should run this scan on a worker, never in a UI callback.
    database.load_system_fonts();
    group_faces(database.faces().filter_map(|face| {
        let (name, _) = face.families.first()?;
        let aliases = face.families.iter().map(|(name, _)| name.clone()).collect();
        Some((
            name.clone(),
            aliases,
            FontFace {
                weight: face.weight.0,
                stretch: face.stretch.to_number() as u8,
                style: match face.style {
                    fontdb::Style::Normal => "Normal",
                    fontdb::Style::Italic => "Italic",
                    fontdb::Style::Oblique => "Oblique",
                }
                .into(),
                postscript_name: face.post_script_name.clone(),
                label: database
                    .with_face_data(face.id, face_label)
                    .flatten()
                    .unwrap_or_else(|| face.post_script_name.clone()),
            },
        ))
    }))
}

fn face_label(bytes: &[u8], index: u32) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, index).ok()?;
    choose_face_label(
        face.names()
            .into_iter()
            .filter_map(|name| {
                Some((
                    name.name_id,
                    name.language() == ttf_parser::Language::English_UnitedStates,
                    name.to_string()?,
                ))
            })
            .collect(),
    )
}

fn choose_face_label(names: Vec<(u16, bool, String)>) -> Option<String> {
    // Typographic subfamily describes the readable face; PostScript identity
    // remains separate. English is a stable fallback until locale services exist.
    for id in [
        ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY,
        ttf_parser::name_id::SUBFAMILY,
    ] {
        for english_only in [true, false] {
            if let Some((_, _, label)) = names.iter().find(|(kind, english, text)| {
                *kind == id
                    && (!english_only || *english)
                    && !text.trim().is_empty()
                    && !text.chars().any(char::is_control)
            }) {
                return Some(label.clone());
            }
        }
    }
    None
}

fn group_faces(
    faces: impl IntoIterator<Item = (String, Vec<String>, FontFace)>,
) -> Vec<FontFamily> {
    let mut families: BTreeMap<String, (BTreeSet<String>, BTreeSet<FontFace>)> = BTreeMap::new();
    for (name, aliases, face) in faces {
        if name.starts_with('.')
            || name.trim().is_empty()
            || !riina_core::subtitle_controls::valid_font_name(&name)
            || !riina_core::subtitle_controls::valid_font_name(&face.postscript_name)
        {
            continue;
        }
        let entry = families.entry(name).or_default();
        entry.0.extend(
            aliases
                .into_iter()
                .filter(|alias| riina_core::subtitle_controls::valid_font_name(alias)),
        );
        // Duplicate files/collection entries must not duplicate picker rows.
        entry.1.insert(face);
    }
    families
        .into_iter()
        .map(|(name, (aliases, faces))| {
            let mut faces: Vec<_> = faces.into_iter().collect();
            // Family selection should prefer a conventional width and upright
            // regular face, rather than a thin or condensed face by sort order.
            faces.sort_by_key(|face| {
                (
                    face.style != "Normal",
                    face.weight.abs_diff(400),
                    face.weight,
                    face.stretch.abs_diff(5),
                    face.postscript_name.clone(),
                )
            });
            FontFamily {
                name,
                aliases: aliases.into_iter().collect(),
                faces,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_face_labels_prefer_typographic_names_and_safe_fallbacks() {
        let record = |id, english, text: &str| (id, english, text.to_owned());
        assert_eq!(
            choose_face_label(vec![
                record(2, true, "Bold"),
                record(17, false, "太字"),
                record(17, true, "Demi Bold")
            ]),
            Some("Demi Bold".into())
        );
        assert_eq!(
            choose_face_label(vec![record(17, true, "\n"), record(2, true, "Regular")]),
            Some("Regular".into())
        );
        assert_eq!(
            choose_face_label(vec![record(17, false, "字体常规")]),
            Some("字体常规".into())
        );
        assert_eq!(
            choose_face_label(vec![
                record(6, true, "Example-Regular"),
                record(2, true, " ")
            ]),
            None
        );
        assert!(face_label(b"not a font", 0).is_none());
    }

    #[test]
    fn families_merge_aliases_and_faces_without_duplicate_source_entries() {
        let normal = FontFace {
            weight: 400,
            stretch: 5,
            style: "Normal".into(),
            postscript_name: "Example-Regular".into(),
            label: "Regular".into(),
        };
        let bold = FontFace {
            weight: 700,
            stretch: 5,
            style: "Normal".into(),
            postscript_name: "Example-Bold".into(),
            label: "Bold".into(),
        };
        let families = group_faces([
            ("Example".into(), vec!["示例".into()], bold.clone()),
            ("Example".into(), vec!["Example".into()], normal.clone()),
            ("Example".into(), vec!["示例".into()], normal.clone()),
            (".Internal".into(), vec![], normal.clone()),
            (" ".into(), vec![], normal.clone()),
            ("Invalid\nname".into(), vec![], normal.clone()),
        ]);
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].faces, [normal, bold]);
        assert_eq!(families[0].aliases, ["Example", "示例"]);
        assert!(families[0].matches("EXAMP"));
        assert!(families[0].matches("示例"));
        assert!(!families[0].matches("absent"));
    }

    #[test]
    fn family_faces_put_regular_before_thin_and_italic_variants() {
        let face = |weight, style: &str, name: &str| FontFace {
            weight,
            stretch: 5,
            style: style.into(),
            postscript_name: name.into(),
            label: name.into(),
        };
        let families = group_faces([
            (
                "Example".into(),
                vec![],
                face(100, "Normal", "Example-Thin"),
            ),
            (
                "Example".into(),
                vec![],
                face(400, "Italic", "Example-Italic"),
            ),
            (
                "Example".into(),
                vec![],
                face(700, "Normal", "Example-Bold"),
            ),
            (
                "Example".into(),
                vec![],
                face(400, "Normal", "Example-Regular"),
            ),
        ]);

        assert_eq!(
            families[0]
                .faces
                .iter()
                .map(|face| face.postscript_name.as_str())
                .collect::<Vec<_>>(),
            [
                "Example-Regular",
                "Example-Thin",
                "Example-Bold",
                "Example-Italic"
            ]
        );
    }

    #[test]
    fn family_faces_prefer_normal_width_before_condensed_variants() {
        let condensed = FontFace {
            weight: 400,
            stretch: 3,
            style: "Normal".into(),
            postscript_name: "Example-Condensed".into(),
            label: "Condensed".into(),
        };
        let regular = FontFace {
            weight: 400,
            stretch: 5,
            style: "Normal".into(),
            postscript_name: "Example-Regular".into(),
            label: "Regular".into(),
        };
        let families = group_faces([
            ("Example".into(), vec![], condensed.clone()),
            ("Example".into(), vec![], regular.clone()),
        ]);
        assert_eq!(families[0].faces, [regular, condensed]);
    }
}
