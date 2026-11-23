//! Opt-in desktop check for the native browser bridge and Settings ownership.
use super::{PluginSettings, PreferencesWindow, register_settings_page};
use crate::{platform, plugin_runtime::PluginPreferences};
use riina_core::plugin::Manifest;
use riina_test_support::TempDir;
use serde_json::{Value, json};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    fs,
    rc::Rc,
    time::{Duration, Instant},
};

const FORM: &str = r#"<!doctype html>
<input id="message" data-pref-key="message">
<input id="enabled" data-pref-key="enabled" type="checkbox">
<input id="limit" data-pref-key="limit" data-type="int" type="number">
<input id="gain" data-pref-key="gain" data-type="float" type="number" step="0.1">
<input id="left" data-pref-key="side" name="side" type="radio" value="left">
<input id="right" name="side" type="radio" value="right">
<script>
const probe = setInterval(() => {
  const field = id => document.getElementById(id);
  if (field('message').value !== 'default' || !field('enabled').checked ||
      field('limit').value !== '3' || field('gain').value !== '1.5' || !field('left').checked) return;
  clearInterval(probe);
  for (const [id, property, value] of [
      ['message', 'value', 'Café "quoted"'], ['enabled', 'checked', false],
      ['limit', 'value', '9'], ['gain', 'value', '2.5'], ['right', 'checked', true]]) {
    const input = field(id);
    input[property] = value;
    input.dispatchEvent(new Event('change'));
  }
  window.iina.preferences.get('keep', value => {
    window.iina.preferences.set('done', value);
  });
}, 30);
</script>"#;

#[test]
#[ignore = "requires a native desktop and WebView runtime"]
fn plugin_settings_page_persists_native_form_changes_on_close()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = TempDir::new("plugin-settings-native")?;
    let plugins = scratch.path().join("Plugins");
    let package = plugins.join("org.example.form.iinaplugin");
    fs::create_dir_all(&package)?;
    fs::write(package.join("settings.html"), FORM)?;
    fs::write(package.join("main.js"), "")?;
    fs::write(
        package.join("Info.json"),
        json!({
            "name": "Plugin Form", "identifier": "org.example.form", "version": "1",
            "author": {"name": "Test"}, "entry": "main.js", "preferencesPage": "settings.html",
            "preferenceDefaults": {
                "message": "default", "enabled": true, "limit": 3,
                "gain": 1.5, "side": "left", "keep": {"values": [1, "quoted\" value"]}
            }
        })
        .to_string(),
    )?;
    let manifest = Manifest::load_from_directory(&package)?;
    let runtime = PluginPreferences::for_package(&package, &manifest)?;
    let path = plugins.join(".preferences/org.example.form.json");
    platform::startup::select_shared_backend(&Default::default())?;
    let owner = PreferencesWindow::new()?;
    let installed = super::super::InstalledPlugin {
        manifest,
        directory: package,
    };
    register_settings_page(
        &owner,
        &PluginSettings {
            plugins,
            preferences_path: scratch.path().join("preferences.json"),
            inventory: Rc::new(RefCell::new(vec![installed])),
            enabled_path: scratch.path().join("plugin-enabled.json"),
            enabled: Rc::default(),
            runtime_available: true,
        },
    );
    platform::show_window(&owner)?;
    owner.set_selected_pane(9);
    owner.invoke_plugin_row_settings(0);

    let weak = owner.as_weak();
    let ready = Rc::new(Cell::new(false));
    let completed = ready.clone();
    let saved = path.clone();
    let timer = Timer::default();
    let started = Instant::now();
    let closing = Cell::new(false);
    timer.start(TimerMode::Repeated, Duration::from_millis(35), move || {
        if closing.get() {
            return;
        }
        let form_updated = matches!(runtime.get("done"), Ok(Value::Object(_)));
        if !form_updated && started.elapsed() < Duration::from_secs(12) {
            return;
        }
        ready.set(form_updated && !saved.exists());
        closing.set(true);
        let close = weak.clone();
        let _ = slint::spawn_local(async move {
            if let Some(owner) = close.upgrade() {
                owner.invoke_close_plugin_settings();
                let _ = owner.hide();
            }
            let _ = slint::quit_event_loop();
        });
    });
    slint::run_event_loop()?;
    timer.stop();
    assert!(completed.get(), "{}", owner.get_plugin_update_status());
    let saved: Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(
        saved,
        json!({
            "message":"Café \"quoted\"", "enabled":false, "limit":9,
            "gain":2.5, "side":"right", "done":{"values":[1,"quoted\" value"]}
        })
    );
    Ok(())
}
