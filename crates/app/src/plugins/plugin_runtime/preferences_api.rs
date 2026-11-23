//! IINA preferences API for package code in either QuickJS realm.
use super::preferences_store::{MAX_PREFERENCE_BYTES, Store};
use riina_core::plugin::Manifest;
use rquickjs::{Ctx, Object, Value, function::Func};
use serde_json::{Map, Value as JsonValue};
use std::{path::Path, sync::Arc};

pub(super) struct PluginPreferences {
    store: Arc<Store>,
    defaults: Map<String, JsonValue>,
}

impl PluginPreferences {
    pub(super) fn for_package(root: &Path, manifest: &Manifest) -> Result<Self, String> {
        Ok(Self {
            store: Store::for_package(root, &manifest.identifier)?,
            defaults: manifest.preference_defaults.clone(),
        })
    }

    #[cfg(test)]
    pub(super) fn temporary() -> Self {
        Self {
            store: Store::temporary(),
            defaults: Map::new(),
        }
    }
}

pub(super) fn install<'js>(
    iina: &Object<'js>,
    preferences: PluginPreferences,
) -> rquickjs::Result<()> {
    let PluginPreferences { store, defaults } = preferences;
    let api = Object::new(iina.ctx().clone())?;
    let read = store.clone();
    api.set(
        "get",
        Func::from(
            move |ctx: Ctx<'js>, key: String| -> rquickjs::Result<Value<'js>> {
                let value = read.get(&key).map_err(js_error)?;
                let Some(value) = value.as_ref().or_else(|| defaults.get(&key)) else {
                    return Ok(Value::new_null(ctx));
                };
                let json =
                    serde_json::to_string(value).map_err(|error| js_error(error.to_string()))?;
                ctx.json_parse(json)
            },
        ),
    )?;
    let write = store.clone();
    api.set(
        "set",
        Func::from(
            move |ctx: Ctx<'js>, key: String, value: Value<'js>| -> rquickjs::Result<()> {
                let json = ctx
                    .json_stringify(value)?
                    .ok_or_else(|| js_error("Plugin preferences require JSON values".into()))?
                    .to_string()?;
                if json.len().saturating_add(key.len()) > MAX_PREFERENCE_BYTES {
                    return Err(js_error("Plugin preferences exceed 1 MiB".into()));
                }
                let value =
                    serde_json::from_str(&json).map_err(|error| js_error(error.to_string()))?;
                write.set(key, value).map_err(js_error)
            },
        ),
    )?;
    api.set("sync", Func::from(move || store.sync().map_err(js_error)))?;
    iina.set("preferences", api)
}

fn js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message(
        "iina.preferences",
        "an available plugin preference",
        message,
    )
}

#[cfg(test)]
mod tests {
    use crate::plugin_runtime::PluginRuntimeSession;
    use riina_core::plugin::Manifest;
    use riina_test_support::TempDir;
    use serde_json::json;
    use std::fs;

    #[test]
    fn player_and_global_preferences_share_values_and_sync_to_their_package() {
        let scratch = TempDir::new("plugin-preference-api").unwrap();
        let plugins = scratch.path().join("Plugins");
        let package = plugins.join("org.example.settings.iinaplugin");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("Info.json"),
            r#"{"name":"Settings","identifier":"org.example.settings","version":"1.0.0",
                "author":{"name":"Test"},"entry":"main.js","globalEntry":"global.js",
                "preferenceDefaults":{"limit":3,"appearance":{"enabled":true}}}"#,
        )
        .unwrap();
        fs::write(
            package.join("global.js"),
            r#"
            const p = iina.preferences;
            if (p.get('limit') !== 3 || p.get('missing') !== null || !p.get('appearance').enabled)
                throw new Error('manifest defaults');
            if (p.set('limit', 5) !== undefined) throw new Error('set returned a value');
        "#,
        )
        .unwrap();
        fs::write(
            package.join("main.js"),
            r#"
            const p = iina.preferences;
            if (p.get('limit') !== 5) throw new Error('global change unavailable');
            p.set('appearance', {enabled:false, labels:['first', 'second']});
            if (p.sync() !== undefined) throw new Error('sync returned a value');
        "#,
        )
        .unwrap();
        let manifest = Manifest::load_from_directory(&package).unwrap();
        let global = PluginRuntimeSession::start_package_global_entry(
            &package,
            &manifest,
            &[],
            Default::default(),
        )
        .unwrap();
        let saved = plugins.join(".preferences/org.example.settings.json");
        assert!(!saved.exists(), "set must leave disk unchanged until sync");
        let player = PluginRuntimeSession::start_package_entry(&package, &manifest, &[]).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(&saved).unwrap()).unwrap(),
            json!({"limit":5, "appearance":{"enabled":false, "labels":["first", "second"]}})
        );
        drop((global, player));

        fs::write(package.join("main.js"), r#"
            const appearance = iina.preferences.get('appearance');
            if (appearance.enabled || appearance.labels[1] !== 'second') throw new Error('saved settings unavailable');
        "#).unwrap();
        PluginRuntimeSession::start_package_entry(&package, &manifest, &[]).unwrap();

        let another = plugins.join("org.example.other.iinaplugin");
        fs::create_dir(&another).unwrap();
        let mut other = manifest;
        other.identifier = "org.example.other".into();
        fs::write(
            another.join("main.js"),
            "if (iina.preferences.get('limit') !== 3) throw 'leaked setting';",
        )
        .unwrap();
        PluginRuntimeSession::start_package_entry(&another, &other, &[]).unwrap();
    }
}
