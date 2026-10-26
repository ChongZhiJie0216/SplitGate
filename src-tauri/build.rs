fn main() {
    // Release builds ask for Administrator (TUN and route changes need it, SPEC "权限").
    // Debug builds keep the default manifest so `tauri dev` also runs from a normal terminal;
    // the UI then shows a "not Administrator" banner and connecting is disabled.
    let mut attrs = tauri_build::Attributes::new();
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        attrs = attrs.windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")),
        );
    }
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
