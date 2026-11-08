#[path = "build_support/icons.rs"]
mod icons;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::path::Path::new("../../assets/branding/graphite.png");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed=build_support/icons.rs");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is missing")?);
    let assets = icons::generate(&std::fs::read(source)?)?;
    std::fs::write(output.join("graphite-icon.rgba"), assets.rgba)?;
    let ico = output.join("graphite.ico");
    std::fs::write(&ico, assets.ico)?;
    // Check the Cargo target, allowing Linux hosts to cross-compile Windows resources too.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon(ico.to_str().ok_or("Icon path is not valid UTF-8")?)
            .set("ProductName", "Graphite")
            .set("FileDescription", "Graphite code editor")
            .compile()?;
    }
    Ok(())
}
