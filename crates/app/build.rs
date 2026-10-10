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
}
