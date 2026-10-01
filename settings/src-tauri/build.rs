use std::path::Path;

fn main() {
    // The installer bundles the tray app from binaries/. `npm run tauri build`
    // puts the real one there first; a plain `cargo build` only needs the file
    // to exist, so create an empty stand-in if it's missing.
    let target = std::env::var("TARGET").expect("cargo sets TARGET");
    let sidecar = format!("binaries/Pixl-{target}.exe");
    if !Path::new(&sidecar).exists() {
        std::fs::create_dir_all("binaries").expect("create binaries/");
        std::fs::write(&sidecar, b"").expect("create tray app stand-in");
    }
    tauri_build::build()
}
