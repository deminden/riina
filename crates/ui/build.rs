use slint_build::DefaultTranslationContext;
use std::env;

#[expect(
    clippy::expect_used,
    reason = "Cargo must fail when UI generation fails"
)]
fn main() {
    println!("cargo:rerun-if-env-changed=SLINT_EMIT_DEBUG_INFO");
    // Widget regression tests locate real controls by accessible label.
    // Keep their lookup metadata in debug builds without enlarging releases.
    let debug_info = env::var("DEBUG").as_deref() == Ok("true")
        || env::var("SLINT_EMIT_DEBUG_INFO").as_deref() == Ok("1");
    slint_build::compile_with_config(
        "ui/player.slint",
        slint_build::CompilerConfiguration::new()
            .with_debug_info(debug_info)
            .with_default_translation_context(DefaultTranslationContext::None)
            .with_bundled_translations("translations"),
    )
    .expect("compile the custom player interface");
}
