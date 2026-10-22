//! Package-confined CommonJS loading with per-session limits.
use super::MAX_ENTRY_BYTES;
use rquickjs::{Ctx, Function, Object, Value, function::Func};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_REQUIRED_MODULES: usize = 256;
const MAX_REQUIRED_MODULE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUIRE_DEPTH: usize = 64;

#[derive(Clone)]
pub(super) struct PluginModuleContext {
    root: PathBuf,
    pub(super) entry: PathBuf,
    pub(super) current_files: Arc<Mutex<Vec<PathBuf>>>,
    budget: Arc<Mutex<ModuleLoadBudget>>,
}

#[derive(Default)]
struct ModuleLoadBudget {
    modules: usize,
    bytes: usize,
}

pub(super) fn read_package_script(
    package_root: &Path,
    entry_path: &Path,
) -> Result<(String, PluginModuleContext), String> {
    let root = package_root
        .canonicalize()
        .map_err(|error| format!("Could not open plugin package: {error}"))?;
    if !root.is_dir() {
        return Err("Plugin package is not a directory".into());
    }
    let entry = resolve_package_file(&root, entry_path).map_err(|error| {
        format!(
            "Could not open plugin entry {}: {error}",
            entry_path.display()
        )
    })?;
    let source = read_plugin_script(&entry)?;
    Ok((
        source,
        PluginModuleContext {
            root,
            entry,
            current_files: Arc::new(Mutex::new(Vec::new())),
            budget: Arc::new(Mutex::new(ModuleLoadBudget::default())),
        },
    ))
}

/// Revalidate containment after installation and before each module load.
fn resolve_package_file(root: &Path, entry_path: &Path) -> Result<PathBuf, String> {
    let entry = root
        .join(entry_path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !entry.starts_with(root) || !entry.is_file() {
        return Err(format!(
            "Plugin entry {} must be a file inside its package",
            entry_path.display()
        ));
    }
    Ok(entry)
}

fn read_plugin_script(entry: &Path) -> Result<String, String> {
    let length = fs::metadata(entry)
        .map_err(|error| format!("Could not inspect plugin entry: {error}"))?
        .len();
    if length > MAX_ENTRY_BYTES as u64 {
        return Err("Plugin entry exceeds the 1 MiB execution limit".into());
    }
    let bytes = read_bounded_plugin_file(entry, "Plugin entry")?;
    let source =
        std::str::from_utf8(&bytes).map_err(|_| "Plugin entry is not valid UTF-8".to_owned())?;
    Ok(source.to_owned())
}

fn read_bounded_plugin_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| format!("Could not read {label}: {error}"))?;
    let mut bytes = Vec::new();
    file.take((MAX_ENTRY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {label}: {error}"))?;
    if bytes.len() > MAX_ENTRY_BYTES {
        return Err(format!("{label} exceeds the 1 MiB file limit"));
    }
    Ok(bytes)
}

fn load_plugin_module<'js>(
    ctx: Ctx<'js>,
    package: &PluginModuleContext,
    requested_path: &str,
) -> Result<Option<(String, Value<'js>)>, String> {
    if requested_path.is_empty() || requested_path.len() > 4096 {
        return Ok(None);
    }
    let (base_file, depth) = {
        let current_files = package
            .current_files
            .lock()
            .map_err(|_| "Plugin module path stack was poisoned".to_owned())?;
        let Some(base_file) = current_files.last().cloned() else {
            return Ok(None);
        };
        (base_file, current_files.len())
    };
    if depth >= MAX_REQUIRE_DEPTH {
        return Err("Plugin module nesting exceeds the 64-file limit".into());
    }
    let Some(parent) = base_file.parent() else {
        return Ok(None);
    };
    let requested = Path::new(requested_path);
    let Ok(module_path) = parent.join(requested).canonicalize() else {
        return Ok(None);
    };
    if !module_path.starts_with(&package.root) || !module_path.is_file() {
        return Ok(None);
    }
    let metadata = fs::metadata(&module_path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_ENTRY_BYTES as u64 {
        return Err("Plugin module exceeds the 1 MiB file limit".into());
    }
    let bytes = read_bounded_plugin_file(&module_path, "Plugin module")?;
    let source =
        std::str::from_utf8(&bytes).map_err(|_| "Plugin module is not valid UTF-8".to_owned())?;
    {
        let mut budget = package
            .budget
            .lock()
            .map_err(|_| "Plugin module load budget was poisoned".to_owned())?;
        if budget.modules >= MAX_REQUIRED_MODULES
            || budget.bytes.saturating_add(bytes.len()) > MAX_REQUIRED_MODULE_BYTES
        {
            return Err("Plugin module load budget exceeded".into());
        }
        budget.modules += 1;
        budget.bytes += bytes.len();
    }
    let canonical_path = fs::canonicalize(&module_path).map_err(|error| error.to_string())?;
    if !canonical_path.starts_with(&package.root) {
        return Ok(None);
    }

    let mut current_files = package
        .current_files
        .lock()
        .map_err(|_| "Plugin module path stack was poisoned".to_owned())?;
    current_files.push(canonical_path.clone());
    drop(current_files);

    let wrapped_source = format!("(function(module) {{\n{source}\nreturn module.exports;\n}})");
    let evaluated = (|| -> rquickjs::Result<Value<'js>> {
        let function: Function<'js> = ctx.eval(wrapped_source.as_str())?;
        let module = Object::new(ctx.clone())?;
        function.call((module,))
    })();

    if let Ok(mut current_files) = package.current_files.lock() {
        current_files.pop();
    }
    let exports = evaluated.map_err(|error| error.to_string())?;
    Ok(Some((
        canonical_path.to_string_lossy().into_owned(),
        exports,
    )))
}

pub(super) fn install<'js>(
    ctx: Ctx<'js>,
    module_context: Option<PluginModuleContext>,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    if let Some(module_context) = module_context {
        globals.set(
            "__riinaRequire",
            Func::from(
                move |ctx: Ctx, path: String| -> rquickjs::Result<Option<String>> {
                    let Some((canonical_path, exports)) =
                        load_plugin_module(ctx.clone(), &module_context, &path).map_err(
                            |error| {
                                rquickjs::Error::new_from_js_message(
                                    "require",
                                    "package-local module",
                                    error,
                                )
                            },
                        )?
                    else {
                        return Ok(None);
                    };
                    ctx.globals().set("__riinaRequiredModule", exports)?;
                    Ok(Some(canonical_path))
                },
            ),
        )?;
        ctx.eval::<(), _>(
            r#"(() => {
            const cache = new Map();
            const requireNative = __riinaRequire;
            globalThis.require = path => {
                const requestedPath = String(path);
                if (cache.has(requestedPath)) return cache.get(requestedPath);
                const modulePath = requireNative(requestedPath);
                if (modulePath == null) return undefined;
                const module = globalThis.__riinaRequiredModule;
                delete globalThis.__riinaRequiredModule;
                cache.set(modulePath, module);
                cache.set(requestedPath, module);
                return module;
            };
            delete globalThis.__riinaRequire;
        })();"#,
        )?;
    }
    Ok(())
}
