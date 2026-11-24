use serde::{Deserialize, Serialize};
use url::Url;

const SERVICE: &str = "Riina HTTP Authentication";

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Origin(String);

#[derive(Deserialize, Serialize)]
pub(super) struct Account {
    pub(super) username: String,
    pub(super) password: String,
}

impl Origin {
    pub(super) fn from_address(address: &str) -> Option<Self> {
        let url = Url::parse(address).ok()?;
        matches!(url.scheme(), "http" | "https").then(|| Self(url.origin().ascii_serialization()))
    }

    pub(super) fn load(&self) -> Result<Option<Account>, String> {
        let entry = keyring::Entry::new(SERVICE, &self.0).map_err(store_error)?;
        let payload = match entry.get_password() {
            Ok(payload) => payload,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(store_error(error)),
        };
        serde_json::from_str(&payload)
            .map(Some)
            .map_err(|_| "The saved sign-in could not be read".to_owned())
    }

    pub(super) fn store(&self, account: &Account) -> Result<(), String> {
        let payload = serde_json::to_string(account)
            .map_err(|_| "The sign-in could not be saved".to_owned())?;
        let entry = keyring::Entry::new(SERVICE, &self.0).map_err(store_error)?;
        entry.set_password(&payload).map_err(store_error)
    }
}

fn store_error(_: keyring::Error) -> String {
    "The system credential store is unavailable".to_owned()
}

pub(super) fn source(address: &str, account: &Account) -> riina_mpv::StreamSource {
    let plain = || riina_mpv::StreamSource {
        address: address.to_owned(),
        authenticated_address: None,
    };
    let Ok(mut authenticated) = Url::parse(address) else {
        return plain();
    };
    if !matches!(authenticated.scheme(), "http" | "https") {
        return plain();
    }
    if !account.username.is_empty() {
        if authenticated.set_username(&account.username).is_err()
            || authenticated.set_password(Some(&account.password)).is_err()
        {
            return plain();
        }
    } else if authenticated.username().is_empty() && authenticated.password().is_none() {
        return plain();
    }
    let mut visible = authenticated.clone();
    let _ = visible.set_username("");
    let _ = visible.set_password(None);
    riina_mpv::StreamSource {
        address: visible.into(),
        authenticated_address: Some(authenticated.into()),
    }
}

pub(super) fn restore(timer: &slint::Timer, window: &riina_ui::OpenUrlWindow, origin: Origin) {
    use slint::ComponentHandle;
    let weak = window.as_weak();
    timer.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(300),
        move || {
            let expected = origin.clone();
            let weak = weak.clone();
            let _ = crate::background::spawn_named("stream-credential-read", move || {
                if let Ok(Some(account)) = expected.load() {
                    let _ = crate::background::to_ui(weak, move |window| {
                        if !window.get_loading()
                            && matches_origin(&window, &expected)
                            && window.get_username().is_empty()
                            && window.get_password().is_empty()
                        {
                            window.set_username(account.username.into());
                            window.set_password(account.password.into());
                        }
                    });
                }
            });
        },
    );
}

pub(super) fn remember(window: &riina_ui::OpenUrlWindow, origin: Origin, account: Account) {
    use slint::ComponentHandle;
    let weak = window.as_weak();
    let _ = crate::background::spawn_named("stream-credential-write", move || {
        if let Err(error) = origin.store(&account) {
            let _ = crate::background::to_ui(weak, move |window| {
                if window.window().is_visible() && matches_origin(&window, &origin) {
                    window.set_message(error.into());
                }
            });
        }
    });
}

fn matches_origin(window: &riina_ui::OpenUrlWindow, expected: &Origin) -> bool {
    let address = window.get_address();
    riina_core::stream_address::stream_address(address.as_str())
        .ok()
        .and_then(|address| Origin::from_address(&address.url))
        .is_some_and(|origin| &origin == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_accounts_follow_an_http_origin_across_its_paths() {
        let first = Origin::from_address("https://user:pass@Example.test/stream?secret=1").unwrap();
        let next = Origin::from_address("https://example.test:443/another").unwrap();
        assert!(first == next);
        assert_eq!(first.0, "https://example.test");
        assert!(Some(first.clone()) != Origin::from_address("https://example.test:8080"));
        assert!(Some(first) != Origin::from_address("http://example.test"));
        assert!(Origin::from_address("rtsp://example.test/live").is_none());
    }
}
