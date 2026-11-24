use std::{env, path::PathBuf};

#[expect(
    clippy::expect_used,
    reason = "Cargo must fail the build when Windows icon generation fails"
)]
fn main() {
    embed_release_notes().expect("embed optional Riina release notes");
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

fn embed_release_notes() -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    let required = |name| env::var(name).map_err(Error::other);
    let manifest = PathBuf::from(required("CARGO_MANIFEST_DIR")?);
    let version = required("CARGO_PKG_VERSION")?;
    let notes = manifest.join("../../docs/release-notes");
    let source = notes.join(format!("{version}.md"));
    let content = match std::fs::read(&source) {
        Ok(content) => {
            println!("cargo:rerun-if-changed={}", source.display());
            content
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            println!("cargo:rerun-if-changed={}", notes.display());
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    std::fs::write(
        PathBuf::from(required("OUT_DIR")?).join("current-release.md"),
        content,
    )
}
