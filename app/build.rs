//! Embeds the Windows icon and version metadata into the executable, so the
//! shipped `shukanji.exe` shows the ShuKanji icon in Explorer and the taskbar
//! rather than the default Rust icon.
//!
//! This needs the resource compiler (`rc.exe`) from the Windows SDK, which ships
//! alongside the MSVC linker. If it is missing the build still succeeds — the
//! exe just keeps the default icon — so a missing SDK never blocks a build.

fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/app-icon.ico");
        resource.set("ProductName", "Shunkanji");
        resource.set("FileDescription", "Shunkanji");
        resource.set("OriginalFilename", "shukanji.exe");
        resource.set("LegalCopyright", "");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed the Windows icon: {error}");
        }
    }
}
