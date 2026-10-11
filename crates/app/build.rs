use std::{env, path::PathBuf};

fn main() {
    // Slint recursively lowers the shortcut expressions. The growing parity
    // interface exceeds the build script's default Windows stack; reserve a
    // bounded compiler stack on every host rather than requiring linker flags.
    std::thread::Builder::new()
        .name("slint-compiler".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            slint_build::compile("ui/player.slint").expect("compile the custom player interface");
        })
        .expect("start the interface compiler")
        .join()
        .expect("finish the interface compiler");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir =
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest path"));
        let icon = manifest_dir.join("../../assets/branding/windows/riina.ico");
        println!("cargo:rerun-if-changed={}", icon.display());
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().expect("ICO path must be valid UTF-8"))
            .compile()
            .expect("embed the Riina Windows application icon");
    }
}
