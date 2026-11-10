use std::{env, fs, path::PathBuf};

// Link against ./libmpv/mpv.lib and put libmpv-2.dll next to the built exe.
fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("libmpv");
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rerun-if-changed=libmpv");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    // OUT_DIR = target/<profile>/build/<pkg>/out -> target/<profile>
    if let Some(profile_dir) = out.ancestors().nth(3) {
        let _ = fs::copy(dir.join("libmpv-2.dll"), profile_dir.join("libmpv-2.dll"));
    }
}
