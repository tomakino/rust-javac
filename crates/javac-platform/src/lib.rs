//! The platform image: the JDK's system classes and modules as one indexed model.
//!
//! Two containers expose the same view:
//!
//! - the JDK's `lib/modules` (jimage), read through `javac-jimage`;
//! - ravac's own `RPI1` image, produced by `cargo xtask gen-platform`, which drops
//!   method bodies (the compiler never reads them) and is what a distribution ships.
//!
//! The view is the one the compiler needs: the module list, resource lookup by
//! `<module>/<path>`, the packages of a module, and the modules of a package.

mod compact;
mod native;

use std::fmt;
use std::io::{self, Read};
use std::path::Path;

pub use compact::compact_class;
pub use native::{ImageBuilder, MAGIC as NATIVE_MAGIC, NativeImage, VERSION as NATIVE_VERSION};

/// jimage magic (0xCAFE_DADA) in the little-endian byte order the JDK writes.
const JIMAGE_MAGIC_LE: [u8; 4] = [0xDA, 0xDA, 0xFE, 0xCA];

/// Errors produced while opening or reading a platform image.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(io::Error),
    /// The file is not a platform image (jimage or RPI1).
    NotAnImage(String),
    /// The RPI1 image version is not supported.
    WrongVersion {
        /// Image path, for the message.
        name: String,
        /// Version field found in the header.
        version: u32,
    },
    /// The image index is truncated or holds nonsensical offsets.
    Corrupted(String),
    /// The underlying jimage reader failed.
    Jimage(javac_jimage::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::NotAnImage(name) => write!(f, "\"{}\" is not a platform image", name),
            Error::WrongVersion { name, version } => write!(
                f,
                "the platform image \"{}\" is not the correct version: {}",
                name, version
            ),
            Error::Corrupted(message) => write!(f, "corrupted platform image: {}", message),
            Error::Jimage(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::Jimage(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<javac_jimage::Error> for Error {
    fn from(err: javac_jimage::Error) -> Error {
        Error::Jimage(err)
    }
}

/// A read-only view of the platform image.
#[derive(Debug)]
pub enum PlatformImage {
    /// The JDK's `lib/modules` (jimage).
    Jimage(javac_jimage::JImage),
    /// Ravac's own stripped image (RPI1).
    Native(NativeImage),
}

impl PlatformImage {
    /// Opens a platform image, detecting the container from its magic.
    pub fn open(path: &Path) -> Result<PlatformImage, Error> {
        let mut file = std::fs::File::open(path)?;
        let mut magic = [0u8; 4];
        if file.read_exact(&mut magic).is_err() {
            return Err(Error::NotAnImage(path.display().to_string()));
        }
        if magic == NATIVE_MAGIC {
            Ok(PlatformImage::Native(NativeImage::open(path)?))
        } else if magic == JIMAGE_MAGIC_LE {
            Ok(PlatformImage::Jimage(javac_jimage::JImage::open(path)?))
        } else {
            Err(Error::NotAnImage(path.display().to_string()))
        }
    }

    /// The module names in image order (Java: `ImageReader.getModuleNames()`).
    pub fn modules(&self) -> &[String] {
        match self {
            PlatformImage::Jimage(image) => image.modules(),
            PlatformImage::Native(image) => image.modules(),
        }
    }

    /// All module resources, named `<module>/<path>`.
    pub fn resources(&self) -> &[String] {
        match self {
            PlatformImage::Jimage(image) => image.resources(),
            PlatformImage::Native(image) => image.resources(),
        }
    }

    /// Returns the bytes of the named resource, or None when it does not exist.
    ///
    /// The name is `<module>/<path>`; a leading slash is accepted.
    pub fn find_resource(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            PlatformImage::Jimage(image) => image.find_resource(name),
            PlatformImage::Native(image) => image.find_resource(name),
        }
    }

    /// The modules that contain the package (Java: `ImageReader.visitPackageLocation`).
    pub fn modules_of_package(&self, package: &str) -> Result<Vec<String>, Error> {
        match self {
            PlatformImage::Jimage(image) => Ok(image.modules_of_package(package)?),
            PlatformImage::Native(image) => image.modules_of_package(package),
        }
    }

    /// The packages of a module (Java: the reverse of the `/packages` tree).
    pub fn packages_of_module(&self, module: &str) -> Result<Vec<String>, Error> {
        match self {
            PlatformImage::Jimage(image) => Ok(image.packages_of_module(module)?),
            PlatformImage::Native(image) => image.packages_of_module(module),
        }
    }
}
