use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread::JoinHandle,
};

#[derive(Debug)]
struct Request {
    head: String,
    body: Vec<u8>,
}
impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }
    fn method(&self) -> &str {
        self.head.split_whitespace().next().unwrap()
    }
}

fn server(
    listener: TcpListener,
    count: usize,
    response: impl Fn(usize) -> String + Send + 'static,
) -> JoinHandle<Vec<Request>> {
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(4);
        let mut requests = Vec::new();
        for index in 0..count {
            let mut stream: TcpStream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                    {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("Server did not receive its request: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                assert!(
                    bytes.len() <= 64 * 1024,
                    "test request headers exceeded bound"
                );
            }
            let headers = String::from_utf8(bytes).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(
                length <= MAX_NETWORK_REQUEST_BYTES,
                "test request body exceeded bound"
            );
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            requests.push(Request {
                head: headers,
                body,
            });
            stream.write_all(response(index).as_bytes()).unwrap();
        }
        requests
    })
}
fn ok() -> String {
    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into()
}
fn redirect(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}
fn run(download: bool, url: &str, options: &serde_json::Value) -> Result<(), String> {
    if download {
        perform_http_download(url, options, &["127.0.0.1".into()]).map(|_| ())
    } else {
        perform_http_request("GET", url, options, &["127.0.0.1".into()]).map(|_| ())
    }
}
#[test]
fn request_and_download_strip_credentials_on_port_change() {
    for download in [false, true] {
        let first = TcpListener::bind("127.0.0.1:0").unwrap();
        let first_url = format!("http://{}/", first.local_addr().unwrap());
        let second = TcpListener::bind("127.0.0.1:0").unwrap();
        let target = format!("http://{}/target", second.local_addr().unwrap());
        let a = server(first, 1, move |_| redirect(&target));
        let b = server(second, 1, |_| ok());
        run(
            download,
            &first_url,
            &serde_json::json!({"headers":{"Authorization":"Bearer dummy","Cookie":"dummy=1"}}),
        )
        .unwrap();
        assert_eq!(
            a.join().unwrap()[0].header("authorization"),
            Some("Bearer dummy")
        );
        let received = b.join().unwrap();
        assert_eq!(received[0].header("authorization"), None);
        assert_eq!(received[0].header("cookie"), None);
    }
}
#[test]
fn same_origin_credentials_remain_and_redirect_exhaustion_fails() {
    for download in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = server(
            listener,
            2,
            |i| if i == 0 { redirect("/next") } else { ok() },
        );
        run(
            download,
            &url,
            &serde_json::json!({"headers":{"Authorization":"Bearer dummy"}}),
        )
        .unwrap();
        assert!(
            worker
                .join()
                .unwrap()
                .iter()
                .all(|r| r.header("authorization") == Some("Bearer dummy"))
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = server(listener, 11, |_| redirect("/loop"));
        assert!(
            run(download, &url, &serde_json::json!({}))
                .unwrap_err()
                .contains("redirect limit")
        );
        assert_eq!(worker.join().unwrap().len(), 11);
    }
}
#[test]
fn redirect_policy_checks_scheme_host_and_effective_port() {
    let url = url::Url::parse("https://example.org/").unwrap();
    for target in ["https://example.org:444/", "https://other.example.org/"] {
        let mut headers = ureq::http::HeaderMap::new();
        headers.insert(
            "authorization",
            ureq::http::HeaderValue::from_static("dummy"),
        );
        redirect_policy(&url, &url::Url::parse(target).unwrap(), &mut headers).unwrap();
        assert!(!headers.contains_key("authorization"));
    }
    let mut headers = ureq::http::HeaderMap::new();
    headers.insert(
        "authorization",
        ureq::http::HeaderValue::from_static("dummy"),
    );
    redirect_policy(
        &url,
        &url::Url::parse("https://example.org:443/").unwrap(),
        &mut headers,
    )
    .unwrap();
    assert!(headers.contains_key("authorization"));
    assert!(
        redirect_policy(
            &url,
            &url::Url::parse("http://example.org/").unwrap(),
            &mut headers
        )
        .is_err()
    );
}
#[test]
fn blocked_redirect_targets_fail_validation_for_both_transports() {
    for download in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = server(listener, 1, |_| redirect("http://unapproved.invalid/"));
        assert!(
            run(download, &url, &serde_json::json!({}))
                .unwrap_err()
                .contains("allowed domains")
        );
        worker.join().unwrap();
    }
}

#[test]
fn redirect_status_method_and_body_matrix_uses_real_requests_for_both_transports() {
    for download in [false, true] {
        for status in [301, 302, 303, 307, 308] {
            for method in ["POST", "PUT", "HEAD"] {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let authority = listener.local_addr().unwrap().to_string();
                let url = format!("http://{authority}/start");
                let worker = server(listener, 2, move |i| {
                    if i == 0 {
                        format!(
                            "HTTP/1.1 {status} Redirect\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                    } else {
                        ok()
                    }
                });
                let mut options = serde_json::json!({"method":method,"headers":{
                    "Host":"untrusted.invalid","Content-Length":"999999",
                    "Authorization":"Bearer dummy","X-Trace":"keep-case",
                    "Content-Encoding":"identity","Content-Language":"en"
                }});
                if method != "HEAD" {
                    options["data"] = serde_json::json!({"q":"A & B"});
                }
                let result = if download {
                    perform_http_download(&url, &options, &["127.0.0.1".into()]).map(|_| ())
                } else {
                    perform_http_request(method, &url, &options, &["127.0.0.1".into()]).map(|_| ())
                };
                let received = worker.join().expect("request recorder failed");
                let context = format!("download={download}, {method}, status={status}");
                result.unwrap_or_else(|e| panic!("{context}: {e}"));
                assert_eq!(received.len(), 2, "{context}");
                let rewrite = (status == 303 && method != "HEAD")
                    || (matches!(status, 301 | 302) && method == "POST");
                assert_eq!(received[0].method(), method, "{context}");
                assert_eq!(
                    received[1].method(),
                    if rewrite { "GET" } else { method },
                    "{context}"
                );
                assert!(
                    received[1]
                        .head
                        .starts_with(&format!("{} /next HTTP/1.1", received[1].method())),
                    "{context}"
                );
                let body = if method == "HEAD" {
                    b"".as_slice()
                } else {
                    b"q=A+%26+B".as_slice()
                };
                assert_eq!(received[0].body, body, "{context}: original encoded body");
                assert_eq!(
                    received[1].body,
                    if rewrite { b"".as_slice() } else { body },
                    "{context}: redirected body"
                );
                for request in &received {
                    assert_eq!(
                        request.header("host"),
                        Some(authority.as_str()),
                        "{context}: target Host"
                    );
                    assert_ne!(
                        request.header("content-length"),
                        Some("999999"),
                        "{context}: framing"
                    );
                    assert_eq!(
                        request.header("authorization"),
                        Some("Bearer dummy"),
                        "{context}: same origin"
                    );
                    assert_eq!(
                        request.header("x-trace"),
                        Some("keep-case"),
                        "{context}: unrelated header"
                    );
                }
                for name in ["content-encoding", "content-language"] {
                    assert_eq!(
                        received[1].header(name),
                        if rewrite {
                            None
                        } else {
                            received[0].header(name)
                        },
                        "{context}: {name}"
                    );
                }
                assert_eq!(
                    received[1].header("content-type"),
                    if rewrite {
                        None
                    } else {
                        received[0].header("content-type")
                    },
                    "{context}: content-type"
                );
            }
        }
    }
}

#[test]
fn credentials_do_not_return_when_redirect_chain_reenters_original_origin() {
    for download in [false, true] {
        let first = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", first.local_addr().unwrap());
        let second = TcpListener::bind("127.0.0.1:0").unwrap();
        let target = format!("http://{}/target", second.local_addr().unwrap());
        let return_url = format!("{url}return");
        let a = server(
            first,
            2,
            move |i| if i == 0 { redirect(&target) } else { ok() },
        );
        let b = server(second, 1, move |_| redirect(&return_url));
        let result = run(
            download,
            &url,
            &serde_json::json!({"headers":{
                "Authorization":"Bearer dummy", "Cookie":"dummy=1", "Proxy-Authorization":"dummy",
                "Origin":"http://private.invalid", "Referer":"http://private.invalid/path", "X-Trace":"retained"
            }}),
        );
        let first_requests = a.join().unwrap();
        let second_requests = b.join().unwrap();
        result.unwrap();
        for name in [
            "authorization",
            "cookie",
            "proxy-authorization",
            "origin",
            "referer",
        ] {
            assert!(first_requests[0].header(name).is_some(), "initial {name}");
            assert_eq!(
                second_requests[0].header(name),
                None,
                "foreign origin {name}"
            );
            assert_eq!(
                first_requests[1].header(name),
                None,
                "returning origin {name}"
            );
        }
        assert_eq!(first_requests[1].header("x-trace"), Some("retained"));
    }
}

#[test]
fn exactly_ten_redirects_succeed_and_non_redirect_status_is_not_followed() {
    for download in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = server(
            listener,
            11,
            |i| if i < 10 { redirect("/next") } else { ok() },
        );
        let result = run(download, &url, &serde_json::json!({}));
        assert_eq!(worker.join().unwrap().len(), 11);
        result.unwrap();
    }
    for status in [200, 304, 404, 500] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = server(listener, 1, move |_| {
            format!(
                "HTTP/1.1 {status} Response\r\nLocation: /must-not-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
        });
        let result =
            perform_http_request("GET", &url, &serde_json::json!({}), &["127.0.0.1".into()]);
        worker.join().unwrap();
        assert_eq!(result.unwrap()["statusCode"], status);
    }
}

#[test]
fn invalid_and_oversized_options_fail_before_network_io() {
    for (options, expected) in [
        (serde_json::json!({"params":[]}), "params must be an object"),
        (serde_json::json!({"data":"raw"}), "data must be an object"),
        (
            serde_json::json!({"headers":[]}),
            "headers must be an object",
        ),
        (
            serde_json::json!({"headers":{"X-Invalid":"a\r\nb"}}),
            "header",
        ),
        (
            serde_json::json!({"data":{"x":"a".repeat(MAX_NETWORK_REQUEST_BYTES)}}),
            "1 MiB",
        ),
    ] {
        for download in [false, true] {
            let error = run(download, "http://127.0.0.1:9/", &options).unwrap_err();
            assert!(
                error.contains(expected),
                "download={download}, expected {expected}, got {error}"
            );
            assert!(
                !error.contains("HTTP request failed"),
                "must reject before a connection attempt"
            );
        }
    }
}
