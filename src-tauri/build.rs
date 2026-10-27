fn main() {
    // v0.18.0: release builds no longer ask for Administrator (`asInvoker`, SPEC "权限"): with the
    // background service installed the service connects; without it the app shows how to get
    // there (install the service, or restart as administrator). Debug builds keep Tauri's default
    // manifest, which behaves the same.
    let mut attrs = tauri_build::Attributes::new();
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        attrs = attrs.windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")),
        );
    }
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
