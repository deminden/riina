//! Bounded network workers report results back to the owning runtime.
use super::{AvailablePermissions, RuntimeCommand};
use crate::{
    plugin_files::PluginFiles,
    plugin_http::{perform_http_download, perform_http_request, perform_xmlrpc_call},
};
use riina_core::plugin::network_url_allowed;
use rquickjs::{Ctx, Object, function::Func};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::SyncSender,
};
const MAX_ACTIVE_NETWORK_REQUESTS: usize = 8;
const MAX_NETWORK_REQUEST_BYTES: usize = 1024 * 1024;

pub(super) struct NetworkWorkerContext {
    pub(super) commands: SyncSender<RuntimeCommand>,
    pub(super) allowed_domains: Vec<String>,
    pub(super) active: Arc<AtomicUsize>,
}

pub(super) fn install<'js>(
    ctx: Ctx<'js>,
    file_system: PluginFiles,
    permissions: AvailablePermissions,
    network: &NetworkWorkerContext,
) -> rquickjs::Result<()> {
    if !permissions.network_request {
        return Ok(());
    }
    let globals = ctx.globals();
    let http = Object::new(ctx.clone())?;
    register_http_requests(&http, network)?;
    register_http_downloads(&http, network, file_system, permissions.file_system)?;
    register_xmlrpc_calls(&http, network)?;
    globals.set("__riinaHttp", http)?;
    ctx.eval::<(), _>(include_str!("network_api.js"))?;
    Ok(())
}

fn register_http_requests(
    http: &Object<'_>,
    network: &NetworkWorkerContext,
) -> rquickjs::Result<()> {
    let network_sender = network.commands.clone();
    let active_requests = network.active.clone();
    let domains = network.allowed_domains.clone();
    let xmlrpc_check_domains = domains.clone();
    http.set(
        "__xmlrpcAllowed",
        Func::from(move |location: String| network_url_allowed(&location, &xmlrpc_check_domains)),
    )?;
    http.set(
        "__request",
        Func::from(
            move |id: u32, method: String, url: String, options: String| -> rquickjs::Result<()> {
                let options: serde_json::Value = serde_json::from_str(&options)
                    .map_err(|error| http_js_error(error.to_string()))?;
                if !network_url_allowed(&url, &domains) {
                    return Err(http_js_error(
                        "Plugin is not allowed to access this URL".into(),
                    ));
                }
                if options.to_string().len() > MAX_NETWORK_REQUEST_BYTES {
                    return Err(http_js_error(
                        "HTTP request options exceed the 1 MiB limit".into(),
                    ));
                }
                let Some(slot) = NetworkRequestSlot::acquire(active_requests.clone()) else {
                    return Err(http_js_error(
                        "Plugin has reached the 8 active HTTP request limit".into(),
                    ));
                };
                let sender = network_sender.clone();
                let domains = domains.clone();
                let worker = crate::background::spawn_named("plugin-http", move || {
                    let result = perform_http_request(&method, &url, &options, &domains);
                    slot.deliver(&sender, RuntimeCommand::DeliverNetworkResult(id, result));
                });
                if let Err(error) = worker {
                    return Err(http_js_error(format!(
                        "Could not start HTTP request: {error}"
                    )));
                }
                Ok(())
            },
        ),
    )?;
    Ok(())
}

fn register_http_downloads(
    http: &Object<'_>,
    network: &NetworkWorkerContext,
    download_files: PluginFiles,
    download_filesystem_granted: bool,
) -> rquickjs::Result<()> {
    let download_sender = network.commands.clone();
    let download_active = network.active.clone();
    let download_domains = network.allowed_domains.clone();
    http.set(
        "__download",
        Func::from(
            move |id: u32,
                  url: String,
                  destination: String,
                  options: String|
                  -> rquickjs::Result<()> {
                let options: serde_json::Value = serde_json::from_str(&options)
                    .map_err(|error| http_js_error(error.to_string()))?;
                if !network_url_allowed(&url, &download_domains) {
                    return Err(http_js_error(
                        "Plugin is not allowed to access this URL".into(),
                    ));
                }
                if options.to_string().len() > MAX_NETWORK_REQUEST_BYTES {
                    return Err(http_js_error(
                        "HTTP request options exceed the 1 MiB limit".into(),
                    ));
                }
                if destination.len() > 16 * 1024 || destination.contains('\0') {
                    return Err(http_js_error("Download destination is invalid".into()));
                }
                let Some(slot) = NetworkRequestSlot::acquire(download_active.clone()) else {
                    return Err(http_js_error(
                        "Plugin has reached the 8 active HTTP request limit".into(),
                    ));
                };
                let sender = download_sender.clone();
                let domains = download_domains.clone();
                let files = download_files.clone();
                let filesystem_granted = download_filesystem_granted;
                let worker = crate::background::spawn_named("plugin-http-download", move || {
                    let result =
                        perform_http_download(&url, &options, &domains).and_then(|content| {
                            files.write_download(&destination, &content, filesystem_granted)
                        });
                    slot.deliver(&sender, RuntimeCommand::DeliverNetworkDownload(id, result));
                });
                if let Err(error) = worker {
                    return Err(http_js_error(format!(
                        "Could not start HTTP download: {error}"
                    )));
                }
                Ok(())
            },
        ),
    )?;
    Ok(())
}

fn register_xmlrpc_calls(
    http: &Object<'_>,
    network: &NetworkWorkerContext,
) -> rquickjs::Result<()> {
    let xmlrpc_sender = network.commands.clone();
    let xmlrpc_active = network.active.clone();
    let xmlrpc_domains = network.allowed_domains.clone();
    http.set(
        "__xmlrpc",
        Func::from(
            move |id: u32,
                  location: String,
                  method: String,
                  args: String|
                  -> rquickjs::Result<()> {
                if !network_url_allowed(&location, &xmlrpc_domains) {
                    return Err(http_js_error(
                        "Plugin is not allowed to access this URL".into(),
                    ));
                }
                if method.is_empty() || method.len() > 1024 || method.chars().any(char::is_control)
                {
                    return Err(http_js_error("XML-RPC method name is invalid".into()));
                }
                let args: serde_json::Value = serde_json::from_str(&args)
                    .map_err(|error| http_js_error(error.to_string()))?;
                if !args.is_array() || args.to_string().len() > MAX_NETWORK_REQUEST_BYTES {
                    return Err(http_js_error(
                        "XML-RPC arguments must be an array no larger than 1 MiB".into(),
                    ));
                }
                let Some(slot) = NetworkRequestSlot::acquire(xmlrpc_active.clone()) else {
                    return Err(http_js_error(
                        "Plugin has reached the 8 active HTTP request limit".into(),
                    ));
                };
                let sender = xmlrpc_sender.clone();
                let domains = xmlrpc_domains.clone();
                let worker = crate::background::spawn_named("plugin-xmlrpc", move || {
                    let result = perform_xmlrpc_call(&location, &method, &args, &domains);
                    slot.deliver(&sender, RuntimeCommand::DeliverXmlRpcResult(id, result));
                });
                if let Err(error) = worker {
                    return Err(http_js_error(format!(
                        "Could not start XML-RPC call: {error}"
                    )));
                }
                Ok(())
            },
        ),
    )?;
    Ok(())
}

fn http_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("HTTP request", "valid allowed request", message)
}

/// A completed request still owns its slot while waiting for the runtime queue.
/// Dropping a failed spawn or disconnected delivery releases it automatically.
struct NetworkRequestSlot(Arc<AtomicUsize>);

impl NetworkRequestSlot {
    fn acquire(active: Arc<AtomicUsize>) -> Option<Self> {
        active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_ACTIVE_NETWORK_REQUESTS).then_some(count + 1)
            })
            .ok()?;
        Some(Self(active))
    }

    fn deliver(self, sender: &SyncSender<RuntimeCommand>, result: RuntimeCommand) {
        let _ = sender.send(result);
    }
}

impl Drop for NetworkRequestSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn response_backpressure_retains_the_network_slot_until_delivery() {
        let active = Arc::new(AtomicUsize::new(0));
        let mut slots = (0..MAX_ACTIVE_NETWORK_REQUESTS)
            .map(|_| NetworkRequestSlot::acquire(active.clone()).unwrap())
            .collect::<Vec<_>>();
        assert!(NetworkRequestSlot::acquire(active.clone()).is_none());
        let slot = slots.pop().unwrap();
        let (sender, receiver) = mpsc::sync_channel(0);
        let worker = std::thread::spawn(move || {
            slot.deliver(&sender, RuntimeCommand::DeliverNetworkDownload(7, Ok(())));
        });
        assert!(NetworkRequestSlot::acquire(active.clone()).is_none());
        assert!(matches!(
            receiver.recv().unwrap(),
            RuntimeCommand::DeliverNetworkDownload(7, Ok(()))
        ));
        worker.join().unwrap();
        assert!(NetworkRequestSlot::acquire(active.clone()).is_some());
        drop(slots);
        assert_eq!(active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn a_disconnected_runtime_releases_the_network_slot() {
        let active = Arc::new(AtomicUsize::new(0));
        let slot = NetworkRequestSlot::acquire(active.clone()).unwrap();
        let (sender, receiver) = mpsc::sync_channel(0);
        drop(receiver);
        slot.deliver(&sender, RuntimeCommand::DeliverNetworkDownload(7, Ok(())));
        assert_eq!(active.load(Ordering::Acquire), 0);
    }
}
