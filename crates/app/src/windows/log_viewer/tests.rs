use super::*;
use riina_test_support::TempDir;
use riina_ui::LogNavigation;
use slint::{Model, platform::WindowEvent};
use std::{error::Error, fs, io::Write};

#[test]
fn viewer_combines_filters_retains_selected_rows_and_snapshots_output() -> Result<(), Box<dyn Error>>
{
    i_slint_backend_testing::init_no_event_loop();
    let dir = TempDir::new("log-viewer")?;
    let preferences = dir.path().join("preferences.json");
    let application = dir.path().join("riina.log");
    let engine = dir.path().join("native-player.log");
    let startup = "2026-11-24T09:11:00Z INFO [player] Device ready";
    let decoder = "[0.2][warn][demux] Device wake";
    fs::write(&application, format!("{startup}\n"))?;
    fs::write(&engine, format!("{decoder}\n[0.3][v][core] packet read\n"))?;
    let window = LogViewerWindow::new()?;
    let mut view = View::new(&window, &preferences);
    view.refresh(&window, true);
    assert_eq!(window.get_rows().row_count(), 3);
    assert_eq!(
        window
            .get_rows()
            .row_data(0)
            .ok_or("Missing startup")?
            .timestamp,
        "2026-11-24T09:11:00Z"
    );

    view.toggle_subsystem(&window, "player");
    view.toggle_subsystem(&window, "demux");
    assert_eq!(window.get_rows().row_count(), 2);
    window.set_search_text("DEMUX".into());
    view.render(&window);
    assert_eq!(window.get_rows().row_count(), 0);
    window.set_search_text("dEvIcE".into());
    window.set_level_index(3);
    view.render(&window);
    assert_eq!(window.get_rows().row_count(), 1);
    assert_eq!(view.export(&window, true), format!("{decoder}\n"));
    assert_eq!(view.export(&window, false).lines().count(), 3);

    window.set_level_index(0);
    view.render(&window);
    view.select(&window, 0, false, false);
    view.navigate(&window, LogNavigation::Next, true);
    assert_eq!(view.copy(-1), format!("{startup}\n{decoder}\n"));
    view.select(&window, 0, true, false);
    assert_eq!(view.copy(-1), format!("{decoder}\n"));
    assert_eq!(view.copy(0), format!("{startup}\n"));

    let mut append = fs::OpenOptions::new().append(true).open(&application)?;
    append.write_all(b"2026-11-24T09:11:01Z WARN [player] Device reset\n")?;
    view.refresh(&window, false);
    assert_eq!(window.get_rows().row_count(), 3);
    assert_eq!(view.copy(-1), format!("{decoder}\n"));
    view.clear_subsystems(&window);
    window.set_search_text("".into());
    view.render(&window);
    view.select_all(&window);
    assert_eq!(view.export(&window, false), view.copy(-1));
    fs::write(&engine, b"[0.1][info][decode] reload\n")?;
    view.refresh(&window, false);
    assert_eq!(view.copy(-1).lines().count(), 2);
    assert_eq!(window.get_rows().row_count(), 3);
    Ok(())
}

#[test]
fn player_commands_share_a_viewer_that_only_polls_while_visible() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let dir = TempDir::new("log-viewer-window")?;
    let preferences = dir.path().join("preferences.json");
    let log = dir.path().join("native-player.log");
    fs::write(&log, b"[0.1][info][core] first\n[0.2][warn][core] par")?;
    let first = PlayerWindow::new()?;
    let second = PlayerWindow::new()?;
    let owner = Rc::new(ApplicationLogViewer::default());
    owner.connect(&first, &preferences, LogLevel::Debug);
    owner.connect(&second, &preferences, LogLevel::Debug);
    assert!(owner.session.borrow().is_none());
    owner.show(None, &preferences, LogLevel::Debug)?;
    let window = owner
        .session
        .borrow()
        .as_ref()
        .ok_or("Viewer did not open")?
        .window
        .clone();
    assert_eq!(window.get_rows().row_count(), 1);
    assert!(owner.timer.running());

    window.window().dispatch_event(WindowEvent::CloseRequested);
    assert!(!owner.timer.running());
    assert!(!window.window().is_visible());
    fs::OpenOptions::new()
        .append(true)
        .open(&log)?
        .write_all(b"tial\n")?;
    i_slint_backend_testing::mock_elapsed_time(REFRESH_INTERVAL);
    assert_eq!(window.get_rows().row_count(), 1);
    second
        .global::<PlayerMenuActions>()
        .invoke_show_log_viewer();
    assert!(window.window().is_visible());
    assert_eq!(window.get_rows().row_count(), 2);
    assert!(window.get_rows().iter().any(|row| row.message == "partial"));
    drop(first);
    assert!(window.window().is_visible());
    window.hide()?;
    i_slint_backend_testing::mock_elapsed_time(REFRESH_INTERVAL);
    assert!(!owner.timer.running());
    Ok(())
}
