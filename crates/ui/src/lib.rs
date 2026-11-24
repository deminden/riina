//! Generated Slint components shared by the player and headless widget tests.

extern crate self as slint;
extern crate slint as slint_runtime;

pub(crate) use slint_runtime::*;
#[doc(hidden)]
pub use slint_runtime::{ComponentHandle, Global, ModelExt};

mod menu_tree;
mod playback_menu;
pub use playback_menu::{PlaybackMenuCatalog, PlaybackMenuOptions, PlaybackMenuRow};

// Slint's generated code uses this namespace for its item-tree menu provider.
// Keep all other imports on the pinned runtime.
pub(crate) mod private_unstable_api {
    pub(crate) use slint_runtime::private_unstable_api::*;

    pub(crate) mod re_exports {
        pub(crate) use crate::menu_tree::MenuFromItemTree;
        pub(crate) use slint_runtime::private_unstable_api::re_exports::*;
    }
}

#[expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "These operations are emitted by the Slint widget generator"
)]
mod generated {
    slint_runtime::include_modules!();
}

pub use generated::*;
