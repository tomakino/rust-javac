//! Platform data embedded in the binary.
//!
//! A packaged `ravac.exe` is a single file: the `modules` image and `ct.sym`
//! are compiled into it with `include_bytes!` and registered here at startup.
//! Development builds leave them unset and fall back to the files next to the
//! executable.

use std::sync::OnceLock;

static MODULES: OnceLock<&'static [u8]> = OnceLock::new();
static CT_SYM: OnceLock<&'static [u8]> = OnceLock::new();

/// Registers the embedded platform data for this process.
///
/// The first call wins; later calls are ignored so a second compiler run in
/// the same process cannot swap the image under an open handle.
pub fn set_embedded_platform(modules: &'static [u8], ct_sym: &'static [u8]) {
    let _ = MODULES.set(modules);
    let _ = CT_SYM.set(ct_sym);
}

/// The embedded `modules` image, when the binary carries one.
pub fn embedded_modules() -> Option<&'static [u8]> {
    MODULES.get().copied()
}

/// The embedded `ct.sym` archive, when the binary carries one.
pub fn embedded_ct_sym() -> Option<&'static [u8]> {
    CT_SYM.get().copied()
}

/// Opens a platform image at `path`, falling back to the embedded copy when the
/// file does not exist.
///
/// The location handlers keep computing `lib/modules` paths even in a
/// single-binary build; this is the one place that turns such a path into the
/// embedded image.
pub fn open_platform_image(
    _path: &std::path::Path,
) -> Result<javac_platform::PlatformImage, javac_platform::Error> {
    // The platform data is compiled into the binary; there is no file fallback.
    let data = embedded_modules().expect("platform data is compiled into the binary");
    javac_platform::PlatformImage::from_bytes(data)
}
