//! Read the heading, paragraph and list blocks used by the bundled release notes.
use riina_ui::{ReleaseNoteKind as Kind, ReleaseNoteRow};

pub(super) fn rows(markdown: &str) -> Vec<ReleaseNoteRow> {
    let mut rows = Vec::new();
    let mut pending = String::new();
    let mut current = Kind::Paragraph;
    let flush = |rows: &mut Vec<ReleaseNoteRow>, pending: &mut String, kind| {
        if !pending.is_empty() {
            rows.push(ReleaseNoteRow {
                kind,
                text: plain_text(pending).into(),
            });
            pending.clear();
        }
    };
    for line in markdown.lines().map(str::trim) {
        if line.starts_with("# ") || line.is_empty() {
            flush(&mut rows, &mut pending, current);
            current = Kind::Paragraph;
        } else if let Some(heading) = line.strip_prefix("## ") {
            flush(&mut rows, &mut pending, current);
            rows.push(ReleaseNoteRow {
                kind: Kind::Heading,
                text: plain_text(heading).into(),
            });
            current = Kind::Paragraph;
        } else if let Some(bullet) = line.strip_prefix("- ") {
            flush(&mut rows, &mut pending, current);
            current = Kind::Bullet;
            pending.push_str(bullet);
        } else {
            if !pending.is_empty() {
                pending.push(' ');
            }
            pending.push_str(line);
        }
    }
    flush(&mut rows, &mut pending, current);
    rows
}

fn plain_text(markdown: &str) -> String {
    let mut output = String::new();
    let mut remaining = markdown;
    while let Some((before, link)) = remaining.split_once('[') {
        if let Some((label, end)) = link.split_once("](")
            && let Some((_, after)) = end.split_once(')')
        {
            output.push_str(before);
            output.push_str(label);
            remaining = after;
        } else {
            break;
        }
    }
    output.push_str(remaining);
    output.replace(['`', '*'], "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_notes_keep_wrapped_list_items_and_review_labels() {
        let input = "# Riina 4.2.1\nPlayback fixes.\n\n## Video\n- Preserve existing\n  settings.\n- Keep the menu.\n\nSee [current status](https://example.test/status).";
        let result = rows(input);
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].text, "Playback fixes.");
        assert_eq!(result[1].kind, Kind::Heading);
        assert_eq!(result[2].kind, Kind::Bullet);
        assert_eq!(result[2].text, "Preserve existing settings.");
        assert_eq!(result[4].text, "See current status.");
        assert!(rows("").is_empty());
    }
}
