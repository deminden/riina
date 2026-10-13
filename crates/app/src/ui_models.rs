//! Preserve model identity when observations have not changed their visible rows.

use slint::{Model, ModelRc, VecModel};

pub(crate) fn set_rows_if_changed<T: Clone + PartialEq + 'static>(
    current: &ModelRc<T>,
    rows: Vec<T>,
    set: impl FnOnce(ModelRc<T>),
) {
    // Native menu models are observed by Slint's platform adapter. Replacing an
    // identical model still rebuilds the menu and interrupts Windows redraw.
    if current.row_count() == rows.len()
        && rows
            .iter()
            .enumerate()
            .all(|(index, row)| current.row_data(index).as_ref() == Some(row))
    {
        return;
    }
    set(ModelRc::new(VecModel::from(rows)));
}

#[cfg(test)]
mod tests {
    use super::set_rows_if_changed;
    use crate::MenuCatalogRow;
    use slint::{Model, ModelRc, VecModel};
    use std::cell::Cell;

    #[test]
    fn repeated_menu_observations_do_not_replace_the_native_model() {
        let rows = vec![MenuCatalogRow {
            token: "2".into(),
            label: "Chapter Two".into(),
            selected: true,
        }];
        let model = ModelRc::new(VecModel::from(rows.clone()));
        let writes = Cell::new(0);
        for _ in 0..120 {
            set_rows_if_changed(&model, rows.clone(), |_| writes.set(writes.get() + 1));
        }
        assert_eq!(writes.get(), 0);
        let mut changed = rows;
        changed[0].selected = false;
        set_rows_if_changed(&model, changed, |next| {
            assert!(!next.row_data(0).unwrap().selected);
            writes.set(writes.get() + 1);
        });
        assert_eq!(writes.get(), 1);
    }

    #[test]
    fn menu_identity_label_order_and_removal_changes_are_published() {
        let row = |token: &str, label: &str| MenuCatalogRow {
            token: token.into(),
            label: label.into(),
            selected: false,
        };
        let rows = vec![row("1", "One"), row("2", "Two")];
        let model = ModelRc::new(VecModel::from(rows.clone()));
        let changes = [
            vec![row("3", "One"), row("2", "Two")],
            vec![row("1", "Renamed"), row("2", "Two")],
            vec![rows[1].clone(), rows[0].clone()],
            vec![rows[0].clone()],
            vec![],
        ];
        for changed in changes {
            let written = Cell::new(false);
            set_rows_if_changed(&model, changed, |_| written.set(true));
            assert!(written.get());
        }
    }
}
