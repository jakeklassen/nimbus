fn main() {
    // GPUI's Windows backend loads icon resource 1 from the executable for the
    // window class, so embedding it here gives the titlebar, taskbar and Alt+Tab
    // the app icon in `cargo run` and release builds alike.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/nimbus.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/nimbus.ico");
        resource.set("ProductName", "Nimbus");
        resource.set("FileDescription", "Nimbus");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed the app icon: {error}");
        }
    }
}
