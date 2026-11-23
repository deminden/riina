//! Connect an inspector to one live realm and retain only its open transcript.
use super::{RuntimeCommand, execution::ExecutionControl};
use crate::diagnostics::Level;
use riina_core::sync::lock;
use rquickjs::{CatchResultExt, Context, Runtime, Value, object::Filter};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, mpsc},
};

const MAX_LINES: usize = 192;
const MAX_LINE_CHARS: usize = 4096;
const MAX_SOURCE_BYTES: usize = 64 * 1024;

#[derive(Clone, Default)]
pub(crate) struct Directory(Arc<Mutex<DirectoryState>>);

#[derive(Default)]
struct DirectoryState {
    revision: u64,
    next: u64,
    realms: BTreeMap<u64, Realm>,
}

#[derive(Clone)]
pub(crate) struct Realm {
    pub(crate) title: String,
    commands: mpsc::SyncSender<RuntimeCommand>,
    output: Arc<Mutex<Output>>,
}

#[derive(Clone)]
pub(super) struct Prepared {
    directory: Directory,
    realm: Realm,
}

pub(super) struct Registration {
    directory: Directory,
    id: u64,
}

impl Directory {
    pub(super) fn prepare(
        &self,
        name: &str,
        global: bool,
        commands: mpsc::SyncSender<RuntimeCommand>,
    ) -> Prepared {
        Prepared {
            directory: self.clone(),
            realm: Realm {
                title: if global {
                    format!("{name} (Global)")
                } else {
                    name.into()
                },
                commands,
                output: Arc::default(),
            },
        }
    }

    pub(crate) fn list(&self, previous: u64) -> Option<(u64, Vec<(u64, String)>)> {
        let state = lock(&self.0);
        (state.revision != previous).then(|| {
            (
                state.revision,
                state
                    .realms
                    .iter()
                    .map(|(id, realm)| (*id, realm.title.clone()))
                    .collect(),
            )
        })
    }

    pub(crate) fn get(&self, id: u64) -> Option<Realm> {
        lock(&self.0).realms.get(&id).cloned()
    }
}

impl Prepared {
    pub(super) fn register(&self) -> Registration {
        let mut state = lock(&self.directory.0);
        state.next += 1;
        let id = state.next;
        state.realms.insert(id, self.realm.clone());
        state.revision = state.revision.wrapping_add(1);
        Registration {
            directory: self.directory.clone(),
            id,
        }
    }

    pub(super) fn log(&self, level: Level, text: &str) {
        let mut output = lock(&self.realm.output);
        if output.attached {
            output.push(Kind::Log(level), text);
        }
    }

    pub(super) fn evaluate(
        &self,
        context: &Context,
        runtime: &Runtime,
        execution: &ExecutionControl,
        request: Request,
    ) {
        let visit = request.visit;
        let mut output = lock(&self.realm.output);
        if !output.attached || output.visit != visit {
            return;
        }
        output.push(Kind::Prompt, request.prompt());
        drop(output);

        execution.begin();
        let result = context.with(|ctx| {
            let result = match request.command {
                Command::Script(source) => ctx.eval::<Value, _>(source).and_then(render_value),
                Command::Globals => ctx
                    .globals()
                    .own_keys::<String>(Filter::new().string())
                    .collect::<rquickjs::Result<Vec<_>>>()
                    .map(|keys| keys.join("\n")),
            };
            result.catch(&ctx).map_err(|error| error.to_string())
        });
        let pending = execution.drain_jobs(runtime);
        let mut output = lock(&self.realm.output);
        if output.attached && output.visit == visit {
            match result {
                Ok(text) => output.push(Kind::Result, &text),
                Err(error) => output.push(Kind::Error, &error),
            }
            if let Err(error) = pending {
                output.push(Kind::Error, &error);
            }
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut state = lock(&self.directory.0);
        if let Some(realm) = state.realms.remove(&self.id) {
            let mut output = lock(&realm.output);
            output.closed = true;
            output.attached = false;
            state.revision = state.revision.wrapping_add(1);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Prompt,
    Result,
    Error,
    Log(Level),
}

#[derive(Clone)]
pub(crate) struct Line {
    pub(crate) kind: Kind,
    pub(crate) text: String,
}

#[derive(Default)]
struct Output {
    attached: bool,
    closed: bool,
    visit: u64,
    revision: u64,
    lines: VecDeque<Line>,
}

impl Output {
    fn push(&mut self, kind: Kind, text: &str) {
        if self.lines.len() == MAX_LINES {
            self.lines.pop_front();
        }
        let end = text
            .char_indices()
            .nth(MAX_LINE_CHARS)
            .map_or(text.len(), |(offset, _)| offset);
        self.lines.push_back(Line {
            kind,
            text: text[..end].into(),
        });
        self.revision = self.revision.wrapping_add(1);
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.revision = self.revision.wrapping_add(1);
    }
}

pub(crate) struct Console {
    realm: Realm,
    visit: u64,
}

impl Realm {
    pub(crate) fn attach(self) -> Option<Console> {
        let mut output = lock(&self.output);
        if output.closed || output.attached {
            return None;
        }
        output.clear();
        output.visit = output.visit.wrapping_add(1);
        output.attached = true;
        let visit = output.visit;
        drop(output);
        Some(Console { realm: self, visit })
    }
}

impl Console {
    pub(crate) fn submit(&self, command: Command) -> Result<(), String> {
        let output = lock(&self.realm.output);
        if !output.attached {
            return Err("The plugin entry has stopped".into());
        }
        if matches!(&command, Command::Script(source) if source.len() > MAX_SOURCE_BYTES) {
            return Err("The command exceeds 64 KiB".into());
        }
        self.realm
            .commands
            .try_send(RuntimeCommand::EvaluateDeveloper(Request {
                visit: self.visit,
                command,
            }))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "The plugin entry is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "The plugin entry has stopped".into(),
            })
    }

    pub(crate) fn lines(&self, previous: u64) -> Option<(u64, Vec<Line>)> {
        let output = lock(&self.realm.output);
        (output.revision != previous)
            .then(|| (output.revision, output.lines.iter().cloned().collect()))
    }

    pub(crate) fn clear(&self) {
        lock(&self.realm.output).clear();
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        let mut output = lock(&self.realm.output);
        output.attached = false;
        output.clear();
    }
}

pub(crate) enum Command {
    Script(String),
    Globals,
}

pub(super) struct Request {
    visit: u64,
    command: Command,
}

impl Request {
    fn prompt(&self) -> &str {
        match &self.command {
            Command::Script(source) => source,
            Command::Globals => "$global",
        }
    }
}

fn render_value(value: Value<'_>) -> rquickjs::Result<String> {
    if value.is_undefined() {
        return Ok("undefined".into());
    }
    let ctx = value.ctx().clone();
    ctx.json_stringify_replacer_space(value, Value::new_null(ctx.clone()), 2)?
        .map_or_else(|| Ok("[JavaScript value]".into()), |text| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_runtime::{PluginRuntimeSession, PluginUi, tests::manifest};
    use riina_test_support::TempDir;
    use std::{fs, io, time::Duration};

    #[test]
    fn developer_console_inspects_the_selected_realm_and_disconnects_on_reload()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = TempDir::new("plugin-developer-tool")?;
        fs::write(package.path().join("main.js"), "globalThis.total = 10;")?;
        fs::write(package.path().join("global.js"), "globalThis.total = 100;")?;
        let mut manifest = manifest("main.js".into());
        manifest.global_entry = Some("global.js".into());
        let ui = PluginUi::default();
        let start_player = || {
            PluginRuntimeSession::start_package_entry_with_player_context(
                package.path(),
                &manifest,
                &[],
                Default::default(),
                None,
                ui.clone(),
            )
            .map_err(io::Error::other)
        };
        let player = start_player()?;
        let global = PluginRuntimeSession::start_package_global_entry(
            package.path(),
            &manifest,
            &[],
            ui.clone(),
        )
        .map_err(io::Error::other)?;
        let (revision, entries) = ui.developer.list(0).ok_or("No developer realms")?;
        let [(player_id, player_title), (global_id, global_title)] = entries.as_slice() else {
            return Err("Expected one player and one global entry".into());
        };
        assert_eq!(player_title, &manifest.name);
        assert_eq!(global_title, &format!("{} (Global)", manifest.name));
        let open = |id| {
            ui.developer
                .get(id)
                .and_then(Realm::attach)
                .ok_or("Missing console")
        };
        let player_console = open(*player_id)?;
        let global_console = open(*global_id)?;
        for command in [
            Command::Script("++total; iina.console.warn('step'); ({total})".into()),
            Command::Script("throw new Error('probe exception')".into()),
            Command::Globals,
        ] {
            player_console.submit(command).map_err(io::Error::other)?;
        }
        global_console
            .submit(Command::Script("total".into()))
            .map_err(io::Error::other)?;
        player
            .flush_events(Duration::from_secs(2))
            .map_err(io::Error::other)?;
        global
            .flush_events(Duration::from_secs(2))
            .map_err(io::Error::other)?;
        let (_, player_lines) = player_console.lines(0).ok_or("Missing player transcript")?;
        assert!(
            player_lines
                .iter()
                .any(|line| line.kind == Kind::Log(Level::Warning) && line.text == "step")
        );
        assert!(
            player_lines
                .iter()
                .any(|line| line.kind == Kind::Result && line.text.contains("\"total\": 11"))
        );
        assert!(
            player_lines
                .iter()
                .any(|line| line.kind == Kind::Error && line.text.contains("probe exception"))
        );
        assert!(
            player_lines
                .iter()
                .any(|line| line.kind == Kind::Result
                    && line.text.lines().any(|name| name == "iina"))
        );
        let (global_revision, global_lines) =
            global_console.lines(0).ok_or("Missing global transcript")?;
        assert_eq!(
            global_lines.last().map(|line| line.text.as_str()),
            Some("100")
        );
        global_console.clear();
        assert!(
            global_console
                .lines(global_revision)
                .is_some_and(|(_, lines)| lines.is_empty())
        );

        drop(player);
        let replacement = start_player()?;
        let (_, entries) = ui.developer.list(revision).ok_or("Missing reload update")?;
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|(id, _)| id != player_id));
        assert!(player_console.submit(Command::Globals).is_err());
        drop(replacement);
        Ok(())
    }
}
