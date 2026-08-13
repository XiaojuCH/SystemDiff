use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("systemdiff.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());

    if env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some("windows".as_ref())
        && env::var_os("CARGO_CFG_TARGET_ENV").as_deref() == Some("msvc".as_ref())
    {
        println!("cargo:rustc-link-arg-bin=systemdiff=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=systemdiff=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
