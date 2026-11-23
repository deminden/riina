//! Keep hook callbacks in the realm and their native registrations in its worker.
use super::{RuntimeCommand, execution::ExecutionControl};
use riina_mpv::{CommandSender, MpvHookContinuation, MpvHookRegistration};
use rquickjs::{Context, Function, Object, function::Func};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, mpsc::SyncSender},
};

const MAX_REALM_HOOKS: u32 = 32;

#[derive(Default)]
pub(super) struct RealmHooks(Rc<RefCell<Vec<MpvHookRegistration>>>);

impl RealmHooks {
    pub(super) fn install(
        &self,
        iina: &Object<'_>,
        player: Option<CommandSender>,
        execution: Arc<ExecutionControl>,
        commands: SyncSender<RuntimeCommand>,
    ) -> rquickjs::Result<()> {
        let registrations = Rc::downgrade(&self.0);
        let register = Func::from(move |name: String, priority: i32| {
            let (player, deadline) = player
                .as_ref()
                .zip(execution.deadline())
                .ok_or_else(|| js_error("The player engine is unavailable"))?;
            let registrations = registrations
                .upgrade()
                .ok_or_else(|| js_error("The plugin has closed"))?;
            let mut registrations = registrations.borrow_mut();
            let index = u32::try_from(registrations.len())
                .ok()
                .filter(|&index| index < MAX_REALM_HOOKS)
                .ok_or_else(|| js_error("The plugin has reached its mpv hook limit"))?;
            let commands = commands.clone();
            let registration = player
                .register_mpv_hook(&name, priority, deadline, move |next| {
                    // A rejected delivery drops its continuation and releases mpv.
                    let _ = commands.try_send(RuntimeCommand::DeliverMpvHook(index, next));
                })
                .map_err(js_error)?;
            registrations.push(registration);
            Ok::<u32, rquickjs::Error>(index)
        });

        let ctx = iina.ctx();
        let mpv: Object<'_> = iina.get("mpv")?;
        let setup: Function<'_> = ctx.eval(include_str!("mpv_hooks.js"))?;
        let deliver: Function<'_> =
            setup.call((mpv, register, iina.get::<_, Function<'_>>("log")?))?;
        ctx.globals().set("__riinaDeliverMpvHook", deliver)
    }
}

pub(super) fn dispatch(
    context: &Context,
    index: u32,
    next: MpvHookContinuation,
) -> Result<(), String> {
    if !next.is_pending() {
        return Ok(());
    }
    context.with(|ctx| {
        let deliver = || {
            let deliver: Function<'_> = ctx.globals().get("__riinaDeliverMpvHook")?;
            deliver.call((index, Func::from(move || next.finish())))
        };
        deliver().map_err(|error| {
            if matches!(error, rquickjs::Error::Exception) {
                ctx.catch();
            }
            error.to_string()
        })
    })
}

fn js_error(error: impl std::fmt::Display) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.mpv.addHook", "mpv hook", error.to_string())
}
