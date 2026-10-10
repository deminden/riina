//! Shared picker lifetime and background font discovery.
use crate::{FontPicker, PlayerWindow, font_catalog};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, ModelRc, StandardListViewItem, VecModel};
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

pub fn connect(app: &PlayerWindow, sender: CommandSender) {
    let current = Rc::new(RefCell::new(None::<FontPicker>));
    let weak = app.as_weak();
    app.on_choose_subtitle_font(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if app.get_dialog_open() {
            return;
        }
        let picker = match FontPicker::new() {
            Ok(picker) => picker,
            Err(error) => {
                app.set_status(error.to_string().into());
                return;
            }
        };
        let font = app.get_subtitle_font();
        picker.set_custom_name(
            if font == riina_core::subtitle_controls::DEFAULT_SUBTITLE_FONT {
                "".into()
            } else {
                font
            },
        );
        let parent = app.as_weak();
        let window = picker.as_weak();
        let commands = sender.clone();
        picker.on_accept_font(move |text| {
            let Some(app) = parent.upgrade() else {
                return;
            };
            match riina_core::subtitle_controls::font_from_text(&text)
                .map_err(str::to_owned)
                .and_then(|command| commands.send(command))
            {
                Ok(()) => {
                    if let Some(window) = window.upgrade() {
                        let _ = window.hide();
                    }
                    app.set_dialog_open(false);
                    app.invoke_activity();
                }
                Err(error) => app.set_status(error.into()),
            }
        });
        let parent = app.as_weak();
        let window = picker.as_weak();
        picker.on_cancel(move || {
            if let Some(window) = window.upgrade() {
                let _ = window.hide();
            }
            if let Some(app) = parent.upgrade() {
                app.set_dialog_open(false);
                app.invoke_activity();
            }
        });
        let parent = app.as_weak();
        picker.window().on_close_requested(move || {
            if let Some(app) = parent.upgrade() {
                app.set_dialog_open(false);
                app.invoke_activity();
            }
            slint::CloseRequestResponse::HideWindow
        });
        if let Err(error) = picker.show() {
            app.set_status(error.to_string().into());
            return;
        }
        app.set_dialog_open(true);
        let window = picker.as_weak();
        *current.borrow_mut() = Some(picker);
        // Only owned metadata crosses threads. The weak window prevents a late
        // scan from reviving a picker replaced or closed with its player.
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
                        let indices: Vec<_> = data
                            .iter()
                            .enumerate()
                            .filter(|(_, family)| family.matches(&text))
                            .map(|(index, _)| index)
                            .collect();
                        picker.set_families(items(
                            indices.iter().map(|index| data[*index].name.clone()),
                        ));
                        *map.borrow_mut() = indices;
                        selection.set(None);
                        picker.set_faces(items(std::iter::empty()));
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
                        picker.set_faces(items(
                            data[index]
                                .faces
                                .iter()
                                .map(|face| face.postscript_name.clone()),
                        ));
                        picker.set_custom_name(data[index].name.clone().into());
                    });
                    let data = families;
                    let weak = picker.as_weak();
                    picker.on_face_selected(move |row| {
                        let Some(face) = selected.get().and_then(|index| {
                            usize::try_from(row)
                                .ok()
                                .and_then(|row| data[index].faces.get(row))
                        }) else {
                            return;
                        };
                        if let Some(picker) = weak.upgrade() {
                            picker.set_custom_name(face.postscript_name.clone().into());
                        }
                    });
                    picker.invoke_search("".into());
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
