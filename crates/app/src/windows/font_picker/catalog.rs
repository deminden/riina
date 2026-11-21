//! Project discovered font families and faces into the picker's live search.
use super::{items, preferred_face_index, preview_face, selection_for_name};
use crate::{FontPicker, font_catalog};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) fn install(picker: &FontPicker, families: Vec<font_catalog::FontFamily>) {
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
            picker.invoke_restore_selection(picker.get_family_index(), face_index as i32);
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
}
