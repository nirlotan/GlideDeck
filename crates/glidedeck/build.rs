//! Embeds the application icon and version metadata into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/glidedeck.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/glidedeck.ico")
            .set("ProductName", "GlideDeck")
            .set("FileDescription", "GlideDeck - local-first zooming presentations")
            .set("CompanyName", "GlideDeck")
            .set("LegalCopyright", "MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            // A missing resource compiler only costs the icon, never the build.
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
