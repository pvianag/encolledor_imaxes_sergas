fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    println!("cargo:rerun-if-changed=assets/app_icon.png");

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    // Embed icon into the .exe when a Windows resource compiler is available
    // (native Windows builds / mingw). Cross-builds without windres still get
    // the runtime window icon from egui.
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/app_icon.ico");
    res.set("ProductName", "Sergas ZIP Shrinker");
    res.set("FileDescription", "Sergas ZIP Shrinker");
    if let Err(err) = res.compile() {
        println!("cargo:warning=Windows icon resource not embedded: {err}");
    }
}
