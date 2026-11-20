use std::env;

#[expect(
    clippy::expect_used,
    reason = "Cargo must fail when UI generation fails"
)]
fn main() {
    println!("cargo:rerun-if-env-changed=SLINT_EMIT_DEBUG_INFO");
    // Slint recursively lowers the shortcut expressions. The growing parity
    // interface exceeds the build script's default Windows stack; reserve a
    // bounded compiler stack on every host rather than requiring linker flags.
    std::thread::Builder::new()
        .name("slint-compiler".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            // Widget regression tests locate real controls by accessible label.
            // Keep their lookup metadata in debug builds without enlarging releases.
            let debug_info = env::var("DEBUG").as_deref() == Ok("true")
                || env::var("SLINT_EMIT_DEBUG_INFO").as_deref() == Ok("1");
            slint_build::compile_with_config(
                "../app/ui/player.slint",
                slint_build::CompilerConfiguration::new().with_debug_info(debug_info),
            )
            .expect("compile the custom player interface");
        })
        .expect("start the interface compiler")
        .join()
        .expect("finish the interface compiler");
}
