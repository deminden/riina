//! Stable plugin ordering shared by the manager and player runtime.

use std::collections::HashSet;

/// Keeps known identifiers in their saved order and appends newly installed IDs.
pub fn normalize(saved: &[String], installed: &[String]) -> Vec<String> {
    let installed_set = installed.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut ordered = saved
        .iter()
        .filter(|identifier| {
            installed_set.contains(identifier.as_str()) && seen.insert(identifier.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    ordered.extend(
        installed
            .iter()
            .filter(|identifier| seen.insert(identifier.as_str()))
            .cloned(),
    );
    ordered
}

/// Moves one installed plugin immediately before the target in a stable order.
pub fn move_before(order: &mut Vec<String>, dragged: &str, target: &str) -> bool {
    if dragged == target {
        return false;
    }
    let Some(from) = order.iter().position(|identifier| identifier == dragged) else {
        return false;
    };
    let Some(to) = order.iter().position(|identifier| identifier == target) else {
        return false;
    };
    let identifier = order.remove(from);
    let target = order
        .iter()
        .position(|candidate| candidate == target)
        .unwrap_or(to.min(order.len()));
    order.insert(target, identifier);
    true
}

#[cfg(test)]
mod tests {
    use super::{move_before, normalize};

    #[test]
    fn normalize_preserves_saved_order_deduplicates_and_appends_new_plugins() {
        let saved = vec!["b".into(), "b".into(), "removed".into(), "a".into()];
        let installed = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(normalize(&saved, &installed), ["b", "a", "c"]);
    }

    #[test]
    fn move_before_reorders_in_both_directions() {
        let mut order = vec!["a".into(), "b".into(), "c".into()];
        assert!(move_before(&mut order, "c", "a"));
        assert_eq!(order, ["c", "a", "b"]);
        assert!(move_before(&mut order, "a", "b"));
        assert_eq!(order, ["c", "a", "b"]);
    }

    #[test]
    fn move_before_rejects_missing_or_identical_plugins() {
        let mut order = vec!["a".into(), "b".into()];
        assert!(!move_before(&mut order, "a", "a"));
        assert!(!move_before(&mut order, "missing", "a"));
        assert!(!move_before(&mut order, "a", "missing"));
        assert_eq!(order, ["a", "b"]);
    }
}
