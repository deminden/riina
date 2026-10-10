//! Installed-font metadata for the shared picker; no font bytes leave discovery.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontFace {
    pub weight: u16,
    pub style: String,
    pub postscript_name: String,
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
                style: match face.style {
                    fontdb::Style::Normal => "Normal",
                    fontdb::Style::Italic => "Italic",
                    fontdb::Style::Oblique => "Oblique",
                }
                .into(),
                postscript_name: face.post_script_name.clone(),
            },
        ))
    }))
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
        .map(|(name, (aliases, faces))| FontFamily {
            name,
            aliases: aliases.into_iter().collect(),
            faces: faces.into_iter().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_merge_aliases_and_faces_without_duplicate_source_entries() {
        let normal = FontFace {
            weight: 400,
            style: "Normal".into(),
            postscript_name: "Example-Regular".into(),
        };
        let bold = FontFace {
            weight: 700,
            style: "Normal".into(),
            postscript_name: "Example-Bold".into(),
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
}
