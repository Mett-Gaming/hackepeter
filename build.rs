//! Version fuer Absturzberichte und Anzeige: in der CI der Tag, sonst `git describe`, sonst
//! Cargo.toml. Dazu moderne Windows-Bedienelemente fuer das Absturzfenster der Mettgine.

use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
    let version = env::var("GITHUB_REF_NAME")
        .ok()
        .filter(|r| r.starts_with('v'))
        .or_else(|| {
            std::process::Command::new("git")
                .args(["describe", "--tags", "--always"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=HACKEPETER_VERSION={version}");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
