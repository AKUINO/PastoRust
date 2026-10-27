use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let assets = [
        ("INDEX_HTML", "web/templates/index.html"),
        ("STYLE_CSS", "web/static/style.css"),
        ("APP_JS", "web/static/app.js"),
    ];
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut embedded_assets = File::create(out.join("web_assets.rs")).unwrap();

    for (constant, asset) in assets {
        println!("cargo:rerun-if-changed={asset}");
        let asset_path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join(asset);
        writeln!(
            embedded_assets,
            "pub const {constant}: &[u8] = include_bytes!({:?});",
            asset_path
        )
        .unwrap();
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    for bin in ["pastorust", "usb-hello", "wifi-led-test", "gpio0-led-test"] {
        println!("cargo:rustc-link-arg-bin={bin}=--nmagic");
        println!("cargo:rustc-link-arg-bin={bin}=-Timage_def.x");
        println!("cargo:rustc-link-arg-bin={bin}=-Tlink.x");
        println!("cargo:rustc-link-arg-bin={bin}=-Tdefmt.x");
    }

    File::create(out.join("image_def.x"))
        .unwrap()
        .write_all(include_bytes!("image_def.x"))
        .unwrap();
    println!("cargo:rerun-if-changed=image_def.x");
}
