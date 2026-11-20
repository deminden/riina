use std::{env, path::PathBuf};

#[expect(
    clippy::expect_used,
    reason = "Cargo must fail the build when Windows icon generation fails"
)]
fn main() {
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
