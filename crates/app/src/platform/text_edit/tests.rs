use super::*;
use slint::ComponentHandle;

slint::slint! {
    #[edition = "1.19"]
    export component Editors inherits Window {
        width: 320px;
        height: 120px;
        in-out property <string> source-text <=> source.text;
        in-out property <string> destination-text <=> destination.text;
        in property <bool> source-read-only;
        callback focus-source;
        callback focus-destination;
        callback focus-popup;
        focus-source => { source.focus(); }
        focus-destination => { destination.focus(); }
        focus-popup => { popup.focus(); }
        source := TextInput {
            x: 5px; y: 5px; width: 250px; height: 30px;
            read-only: root.source-read-only;
            text: "Café 🙂";
        }
        destination := TextInput {
            x: 5px; y: 50px; width: 250px; height: 30px;
        }
        popup := FocusScope {}
    }
}

#[test]
fn menu_focus_handoff_preserves_the_text_selection() {
    i_slint_backend_testing::init_no_event_loop();
    let component = Editors::new().unwrap();
    component.show().unwrap();
    let editor = Editor::new(component.window());
    component.invoke_focus_source();
    assert!(editor.perform(Edit::SelectAll, true));

    // Windows retains the focused item while the framed owner handles its menu.
    component
        .window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(false));
    assert!(editor.perform(Edit::Copy, false));
    component
        .window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // The in-client menu takes focus and then restores the same item and range.
    let focus = PopupFocus::capture(component.window()).unwrap();
    component.invoke_focus_popup();
    assert!(!editor.has_text_focus());
    assert!(focus.restore(component.window()));
    assert!(editor.perform(Edit::Copy, true));
    component.invoke_focus_destination();
    assert!(editor.perform(Edit::Paste, true));
    assert_eq!(component.get_destination_text(), "Café 🙂");
}

#[test]
fn edit_menu_uses_the_focused_text_input_and_its_clipboard_and_history() {
    i_slint_backend_testing::init_no_event_loop();
    let component = Editors::new().unwrap();
    component.show().unwrap();
    let editor = Editor::new(component.window());
    assert!(editor.is_live());
    assert!(!editor.perform(Edit::Paste, false));

    component.set_source_read_only(true);
    component.invoke_focus_source();
    assert!(editor.perform(Edit::SelectAll, true));
    assert!(!editor.perform(Edit::Cut, true));
    assert!(!editor.perform(Edit::Delete, false));
    assert!(editor.perform(Edit::Copy, true));
    assert_eq!(component.get_source_text(), "Café 🙂");

    component.invoke_focus_destination();
    assert!(editor.perform(Edit::Paste, true));
    assert_eq!(component.get_destination_text(), "Café 🙂");
    assert!(editor.perform(Edit::Undo, true));
    assert!(component.get_destination_text().is_empty());
    assert!(editor.perform(Edit::Redo, true));
    assert_eq!(component.get_destination_text(), "Café 🙂");
    assert!(!editor.perform(Edit::Delete, true));
    assert!(editor.perform(Edit::SelectAll, true));
    assert!(editor.perform(Edit::Delete, true));
    assert!(component.get_destination_text().is_empty());

    component.hide().unwrap();
    drop(component);
    assert!(!editor.is_live());
    assert!(!editor.perform(Edit::Paste, true));
}
