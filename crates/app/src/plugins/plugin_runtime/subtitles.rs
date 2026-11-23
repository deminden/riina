//! A subtitle search retains opaque items in the owning player realm.
use super::{AvailablePermissions, PluginRuntimeSession, RuntimeCommand};
use crate::plugin_files::PluginFiles;
use rquickjs::{Context, Function, Object, function::Func};
use serde::Deserialize;
use std::{path::PathBuf, sync::mpsc::SyncSender, time::Instant};

#[derive(Debug, Deserialize)]
pub(crate) struct Description {
    pub(crate) name: String,
    pub(crate) left: String,
    pub(crate) right: String,
}

pub(crate) enum Operation {
    Search {
        provider: String,
        reply: SyncSender<Result<Option<Vec<Description>>, String>>,
    },
    Download {
        indices: Vec<usize>,
        reply: SyncSender<Result<Vec<PathBuf>, String>>,
    },
}

pub(crate) struct Job {
    pub(crate) operation: Operation,
    pub(crate) ticket: u64,
    pub(crate) media_generation: u64,
    pub(crate) deadline: Instant,
}

impl Job {
    pub(crate) fn fail(self, error: String) {
        match self.operation {
            Operation::Search { reply, .. } => _ = reply.try_send(Err(error)),
            Operation::Download { reply, .. } => _ = reply.try_send(Err(error)),
        }
    }
}

impl PluginRuntimeSession {
    pub(crate) fn run_subtitle_provider(&self, job: Job) {
        use std::sync::mpsc::TrySendError;
        if let Err(TrySendError::Full(command) | TrySendError::Disconnected(command)) = self
            .commands
            .try_send(RuntimeCommand::RunSubtitleProvider(job))
            && let RuntimeCommand::RunSubtitleProvider(job) = command
        {
            job.fail("The subtitle provider is unavailable or busy".into());
        }
    }
}

pub(super) struct Realm {
    files: PluginFiles,
    file_system_granted: bool,
}

impl Realm {
    pub(super) fn new(files: PluginFiles, permissions: AvailablePermissions) -> Self {
        Self {
            files,
            file_system_granted: permissions.file_system,
        }
    }

    pub(super) fn install(api: &Object<'_>) -> rquickjs::Result<()> {
        let ctx = api.ctx();
        let setup: Function<'_> = ctx.eval(include_str!("subtitles.js"))?;
        let dispatch: Object<'_> = setup.call((api.clone(),))?;
        ctx.globals().set("__riinaSubtitles", dispatch)
    }

    pub(super) fn dispatch(&self, context: &Context, job: Job) -> Result<(), String> {
        if Instant::now() >= job.deadline || !self.files.is_media_generation(job.media_generation) {
            job.fail("The subtitle search has expired or the media has changed".into());
            return Ok(());
        }
        context.with(|ctx| {
            let send_failure = |error: rquickjs::Error| {
                if error.is_exception() {
                    ctx.catch();
                }
                error.to_string()
            };
            let ticket = job.ticket.to_string();
            let dispatch: Object<'_> = ctx
                .globals()
                .get("__riinaSubtitles")
                .map_err(send_failure)?;
            let result = match job.operation {
                Operation::Search { provider, reply } => {
                    let files = self.files.clone();
                    let complete =
                        Func::from(move |error: Option<String>, json: Option<String>| {
                            let result = completion(error, json).and_then(|json| {
                                verify_current(&files, job.media_generation, job.deadline)?;
                                serde_json::from_str(&json).map_err(|error| error.to_string())
                            });
                            _ = reply.try_send(result);
                        });
                    dispatch
                        .get::<_, Function<'_>>("search")
                        .and_then(|search| search.call((ticket, provider, complete)))
                }
                Operation::Download { indices, reply } => {
                    let files = self.files.clone();
                    let granted = self.file_system_granted;
                    let complete =
                        Func::from(move |error: Option<String>, json: Option<String>| {
                            let result = completion(error, json).and_then(|json| {
                                verify_current(&files, job.media_generation, job.deadline)?;
                                let paths = serde_json::from_str::<Vec<String>>(&json)
                                    .map_err(|error| error.to_string())?;
                                paths
                                    .iter()
                                    .map(|path| files.existing_path(path, granted))
                                    .collect()
                            });
                            _ = reply.try_send(result);
                        });
                    dispatch
                        .get::<_, Function<'_>>("download")
                        .and_then(|download| download.call((ticket, indices, complete)))
                }
            };
            result.map_err(send_failure)
        })
    }
}

fn completion(error: Option<String>, json: Option<String>) -> Result<String, String> {
    if let Some(error) = error {
        return Err(error.chars().take(512).collect());
    }
    json.filter(|json| json.len() <= 1024 * 1024)
        .ok_or_else(|| "The subtitle provider response is missing or exceeds 1 MiB".into())
}

fn verify_current(files: &PluginFiles, generation: u64, deadline: Instant) -> Result<(), String> {
    if Instant::now() < deadline && files.is_media_generation(generation) {
        Ok(())
    } else {
        Err("The subtitle search has expired or the media has changed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_files::PluginPlayerFileContext;
    use std::{sync::mpsc, time::Duration};

    const TIMEOUT: Duration = Duration::from_secs(3);

    #[test]
    fn searches_and_downloads_the_same_opaque_items_under_the_player_scope() {
        let session = PluginRuntimeSession::start_entry(
            r#"
            const original = {value: Symbol('opaque')};
            const pause = () => new Promise(resolve => setTimeout(resolve, 1));
            iina.subtitle.registerProvider('demo', {
                async search() {
                    await pause();
                    return [iina.subtitle.item(original),
                        iina.subtitle.item({}, {name:'Outside'})];
                },
                description() { return {name:'First', left:'English', right:'SRT'}; },
                async download(item) {
                    await pause();
                    if (item.desc.name === 'Outside') return ['@data/../private.srt'];
                    if (item.data !== original) throw new Error('Opaque data was replaced');
                    iina.file.write('@data/demo.srt', 'fixture contents');
                    return ['@data/demo.srt'];
                }
            });
            iina.subtitle.registerProvider('custom', {
                async search() { return iina.subtitle.CUSTOM_IMPLEMENTATION; },
                async download() { return []; }
            });
            "#,
            "org.example.subtitles-test",
            &[],
            &[],
        )
        .unwrap();
        let job = |operation, ticket, media_generation| Job {
            operation,
            ticket,
            media_generation,
            deadline: Instant::now() + TIMEOUT,
        };
        let search = |provider: &str, ticket| {
            let (reply, response) = mpsc::sync_channel(1);
            session.run_subtitle_provider(job(
                Operation::Search {
                    provider: provider.into(),
                    reply,
                },
                ticket,
                0,
            ));
            response.recv_timeout(TIMEOUT).unwrap().unwrap()
        };
        let download = |indices, ticket| {
            let (reply, response) = mpsc::sync_channel(1);
            session.run_subtitle_provider(job(Operation::Download { indices, reply }, ticket, 0));
            response.recv_timeout(TIMEOUT).unwrap()
        };

        let rows = search("demo", 4).unwrap();
        assert_eq!(
            (
                rows[0].name.as_str(),
                rows[0].left.as_str(),
                rows[0].right.as_str()
            ),
            ("First", "English", "SRT")
        );
        assert_eq!(rows[1].name, "Outside");
        let files = download(vec![0], 4).unwrap();
        assert_eq!(
            std::fs::read_to_string(&files[0]).unwrap(),
            "fixture contents"
        );
        assert!(download(vec![1], 4).is_err());
        assert!(search("custom", 5).is_none());
        assert!(download(vec![0], 4).unwrap_err().contains("expired"));
        session.update_player_file_context(
            PluginPlayerFileContext::default().with_media_generation(1),
        );
        assert!(
            download(vec![0], 5)
                .unwrap_err()
                .contains("media has changed")
        );
    }
}
