fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/resources/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        winresource::WindowsResource::new()
            .set_icon("src/resources/icon.ico")
            .compile()?;
    }

    Ok(())
}
