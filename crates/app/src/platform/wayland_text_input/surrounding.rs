//! Convert Wayland byte ranges into WebKit editing ranges.

const TEXT_LIMIT: usize = 4_000;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::platform) struct Surrounding {
    pub(super) text: String,
    pub(super) cursor: i32,
    pub(super) anchor: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::platform) struct Deletion {
    pub(in crate::platform) offset: i32,
    pub(in crate::platform) length: u32,
}

impl Surrounding {
    pub(in crate::platform) fn new(raw: &str, cursor: usize, anchor: usize) -> Option<Self> {
        let start = cursor.min(anchor);
        let end = cursor.max(anchor);
        if !raw.is_char_boundary(start)
            || !raw.is_char_boundary(end)
            || end - start > TEXT_LIMIT
            || raw.contains('\0')
        {
            return None;
        }
        let budget = TEXT_LIMIT - (end - start);
        let right = (raw.len() - end).min(budget / 2);
        let left = start.min(budget - right);
        let right = (raw.len() - end).min(budget - left);
        let low = raw.ceil_char_boundary(start - left);
        let high = raw.floor_char_boundary(end + right);
        Some(Self {
            text: raw[low..high].to_owned(),
            cursor: (cursor - low) as i32,
            anchor: (anchor - low) as i32,
        })
    }

    pub(in crate::platform) fn deletion(
        &self,
        (before, after): (u32, u32),
        replaces_selection: bool,
    ) -> Option<Deletion> {
        let first = self.cursor.min(self.anchor) as usize;
        let last = self.cursor.max(self.anchor) as usize;
        // WebKit can remove a contiguous range. A replacement also consumes
        // the selection, while a deletion alone must leave selected text intact.
        if first != last && !replaces_selection {
            return None;
        }
        let start = first.checked_sub(before as usize)?;
        let end = last.checked_add(after as usize)?;
        let prefix = self.text.get(start..first)?;
        let removed = self.text.get(start..end)?;
        Some(Deletion {
            // WebKit's native delete-surrounding signal counts UTF-16 units.
            offset: -(prefix.encode_utf16().count() as i32),
            length: removed.encode_utf16().count() as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrounding_text_keeps_the_selection_and_bounds_utf8() {
        let raw = format!("{}選択{}", "🪷".repeat(1_400), "日".repeat(1_400));
        let cursor = 5_600;
        let context = Surrounding::new(&raw, cursor, cursor + "選択".len());
        assert!(context.as_ref().is_some_and(|context| {
            context.text.len() <= TEXT_LIMIT
                && &context.text[context.cursor as usize..context.anchor as usize] == "選択"
        }));
        assert!(Surrounding::new(&raw, 0, 5_600).is_none());
        assert!(Surrounding::new(&raw, 2, 5_600).is_none());
    }

    #[test]
    fn converts_both_sides_and_rejects_split_characters_or_unavailable_text() {
        let text = Surrounding::new("a🪷名é", "a🪷".len(), "a🪷".len());
        assert_eq!(
            text.as_ref().and_then(|text| text.deletion((4, 3), false)),
            Some(Deletion {
                offset: -2,
                length: 3
            })
        );
        for range in [(1, 3), (4, 1), (7, 0), (0, 8)] {
            assert!(
                text.as_ref()
                    .and_then(|text| text.deletion(range, false))
                    .is_none()
            );
        }
        let selected = Surrounding::new("a🪷選択名z", 5, 11);
        assert_eq!(
            selected
                .as_ref()
                .and_then(|text| text.deletion((4, 3), true)),
            Some(Deletion {
                offset: -2,
                length: 5
            })
        );
        assert!(
            selected
                .as_ref()
                .and_then(|text| text.deletion((4, 3), false))
                .is_none()
        );
    }
}
