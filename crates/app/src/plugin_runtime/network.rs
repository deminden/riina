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
    let globals = ctx.globals();
    let network_request_allowed = permissions.network_request;
    let network_sender_for_api = network.commands.clone();
    let network_active_for_api = network.active.clone();
    let allowed_domains_for_api = network.allowed_domains.clone();
    if network_request_allowed {
        let http = Object::new(ctx.clone())?;
        let network_sender = network_sender_for_api.clone();
        let active_requests = network_active_for_api.clone();
        let domains = allowed_domains_for_api.clone();
        let download_files = file_system.clone();
        let download_filesystem_granted = permissions.file_system;
        let xmlrpc_check_domains = allowed_domains_for_api.clone();
        http.set(
            "__xmlrpcAllowed",
            Func::from(move |location: String| {
                network_url_allowed(&location, &xmlrpc_check_domains)
            }),
        )?;
        http.set(
            "__request",
            Func::from(
                move |id: u32,
                      method: String,
                      url: String,
                      options: String|
                      -> rquickjs::Result<()> {
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
                    let worker = std::thread::Builder::new()
                        .name("plugin-http".into())
                        .spawn(move || {
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
        let download_sender = network_sender_for_api.clone();
        let download_active = network_active_for_api.clone();
        let download_domains = allowed_domains_for_api.clone();
        let download_files = download_files.clone();
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
                    let worker = std::thread::Builder::new()
                        .name("plugin-http-download".into())
                        .spawn(move || {
                            let result = perform_http_download(&url, &options, &domains).and_then(
                                |content| {
                                    files.write_download(&destination, &content, filesystem_granted)
                                },
                            );
                            slot.deliver(
                                &sender,
                                RuntimeCommand::DeliverNetworkDownload(id, result),
                            );
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
        let xmlrpc_sender = network_sender_for_api.clone();
        let xmlrpc_active = network_active_for_api.clone();
        let xmlrpc_domains = allowed_domains_for_api.clone();
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
                    if method.is_empty()
                        || method.len() > 1024
                        || method.chars().any(char::is_control)
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
                    let worker = std::thread::Builder::new()
                        .name("plugin-xmlrpc".into())
                        .spawn(move || {
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
        globals.set("__riinaHttp", http)?;
        ctx.eval::<(), _>(
        r#"(() => {
            const nativeRequest = __riinaHttp.__request;
            const nativeDownload = __riinaHttp.__download;
            const nativeXmlrpc = __riinaHttp.__xmlrpc;
            const xmlrpcAllowed = __riinaHttp.__xmlrpcAllowed;
            const pending = new Map();
            let nextId = 0;
            const request = (method, url, options = {}) => new Promise((resolve, reject) => {
                nextId = (nextId + 1) >>> 0;
                const id = nextId;
                pending.set(id, { resolve, reject });
                try {
                    nativeRequest(id, method, String(url), JSON.stringify(options));
                } catch (error) {
                    pending.delete(id);
                    reject(error);
                }
            });
            iina.http = {
                get: (url, options) => request('GET', url, options),
                post: (url, options) => request('POST', url, options),
                put: (url, options) => request('PUT', url, options),
                patch: (url, options) => request('PATCH', url, options),
                delete: (url, options) => request('DELETE', url, options),
                download: (url, destination, options = {}) => new Promise((resolve, reject) => {
                    nextId = (nextId + 1) >>> 0;
                    const id = nextId;
                    pending.set(id, { resolve, reject });
                    try {
                        nativeDownload(id, String(url), String(destination), JSON.stringify(options));
                    } catch (error) {
                        pending.delete(id);
                        reject(error);
                    }
                }),
                xmlrpc: location => {
                    const endpoint = String(location);
                    if (!xmlrpcAllowed(endpoint)) throw new Error(`URL ${endpoint} is not allowed.`);
                    return {
                        call: (method, args = []) => new Promise((resolve, reject) => {
                            nextId = (nextId + 1) >>> 0;
                            const id = nextId;
                            pending.set(id, { resolve, reject });
                            try {
                                const json = JSON.stringify(args, function (key, value) {
                                    const original = this && this[key];
                                    if (original instanceof Date) return { __riinaXmlrpcDate: original.toISOString() };
                                    if (ArrayBuffer.isView(original)) return { __riinaXmlrpcBinary: Array.from(original) };
                                    return value;
                                });
                                nativeXmlrpc(id, endpoint, String(method), json);
                            } catch (error) {
                                pending.delete(id);
                                reject(error);
                            }
                        })
                    };
                }
            };
            globalThis.__riinaDeliverHttp = (id, response, error) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                if (error !== null) callbacks.reject(new Error(error));
                else if (response.statusCode < 400 || response.statusCode >= 600) callbacks.resolve(response);
                else callbacks.reject(response);
            };
            globalThis.__riinaDeliverDownload = (id, error) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                if (error !== null) callbacks.reject(new Error(error));
                else callbacks.resolve();
            };
            globalThis.__riinaDeliverXmlrpc = (id, response, error) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                if (error !== null) callbacks.reject(new Error(error));
                else {
                    const hydrate = value => {
                        if (Array.isArray(value)) return value.map(hydrate);
                        if (value && typeof value === 'object') {
                            if (Array.isArray(value.__riinaXmlrpcBinary)) return new Uint8Array(value.__riinaXmlrpcBinary);
                            if (typeof value.__riinaXmlrpcDate === 'string') return new Date(value.__riinaXmlrpcDate);
                            for (const key of Object.keys(value)) value[key] = hydrate(value[key]);
                        }
                        return value;
                    };
                    callbacks.resolve(hydrate(response));
                }
            };
            globalThis.__riinaHttp = undefined;
        })();"#,
    )?;
    }
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
