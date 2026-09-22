fn main() {
    // NOTE: the Windows manifest (requireAdministrator) is NOT embedded
    // here — tauri-build's WindowsAttributes only handles Tauri's
    // default (asInvoker) manifest correctly. The admin manifest is
    // injected into the final exe with mt.exe in build.ps1:
    //   mt.exe -manifest installer-manifest.xml
    //        -outputresource:target\release\glitchy-web-setup.exe;1
    tauri_build::build()
}
