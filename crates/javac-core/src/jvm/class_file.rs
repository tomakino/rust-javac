// Origin: com/sun/tools/javac/jvm/ClassFile.java @ 982d53c (jdk-17.0.19-ga)

//! A JVM class file.
//!
//! <p>Generic Java classfiles have one additional attribute for classes,
//! methods and fields:
//! <pre>
//!  "Signature" (u4 attr-length, u2 signature-index)
//! </pre>
//! ... (see upstream Javadoc for the complete grammars)
//!
//! <p>This class defines constants used in class files as well
//! as routines to convert between internal "." and external "/" separators
//! in class names.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

pub const JAVA_MAGIC: i32 = 0xCAFE_BABE_u32 as i32;

// see Target
pub const CONSTANT_UTF8: i32 = 1;
pub const CONSTANT_UNICODE: i32 = 2;
pub const CONSTANT_INTEGER: i32 = 3;
pub const CONSTANT_FLOAT: i32 = 4;
pub const CONSTANT_LONG: i32 = 5;
pub const CONSTANT_DOUBLE: i32 = 6;
pub const CONSTANT_CLASS: i32 = 7;
pub const CONSTANT_STRING: i32 = 8;
pub const CONSTANT_FIELDREF: i32 = 9;
pub const CONSTANT_METHODREF: i32 = 10;
pub const CONSTANT_INTERFACEMETHODREF: i32 = 11;
pub const CONSTANT_NAMEANDTYPE: i32 = 12;
pub const CONSTANT_METHODHANDLE: i32 = 15;
pub const CONSTANT_METHODTYPE: i32 = 16;
pub const CONSTANT_DYNAMIC: i32 = 17;
pub const CONSTANT_INVOKEDYNAMIC: i32 = 18;
pub const CONSTANT_MODULE: i32 = 19;
pub const CONSTANT_PACKAGE: i32 = 20;

pub const REF_GETFIELD: i32 = 1;
pub const REF_GETSTATIC: i32 = 2;
pub const REF_PUTFIELD: i32 = 3;
pub const REF_PUTSTATIC: i32 = 4;
pub const REF_INVOKEVIRTUAL: i32 = 5;
pub const REF_INVOKESTATIC: i32 = 6;
pub const REF_INVOKESPECIAL: i32 = 7;
pub const REF_NEWINVOKESPECIAL: i32 = 8;
pub const REF_INVOKEINTERFACE: i32 = 9;

pub const MAX_PARAMETERS: i32 = 0xff;
pub const MAX_DIMENSIONS: i32 = 0xff;
pub const MAX_CODE: i32 = 0xffff;
pub const MAX_LOCALS: i32 = 0xffff;
pub const MAX_STACK: i32 = 0xffff;

pub const PREVIEW_MINOR_VERSION: i32 = 0xffff;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    V45_3,
    V49,
    V50,
    V51,
    V52,
    V53,
    V54,
    V55,
    V56,
    V57,
    V58,
    V59,
    V60,
    V61,
}

impl Version {
    /// Java: public static Version MIN() { return MIN; }
    pub const fn min() -> Version {
        Version::V45_3
    }

    /// Java: public static Version MAX() { return MAX; }
    pub const fn max() -> Version {
        Version::V61
    }

    /// Java: public final int major, minor;
    pub const fn major(self) -> i32 {
        match self {
            Version::V45_3 => 45,
            Version::V49 => 49,
            Version::V50 => 50,
            Version::V51 => 51,
            Version::V52 => 52,
            Version::V53 => 53,
            Version::V54 => 54,
            Version::V55 => 55,
            Version::V56 => 56,
            Version::V57 => 57,
            Version::V58 => 58,
            Version::V59 => 59,
            Version::V60 => 60,
            Version::V61 => 61,
        }
    }

    pub const fn minor(self) -> i32 {
        match self {
            Version::V45_3 => 3,
            _ => 0,
        }
    }
}

// ------------------------------------------------------------------------
// String Translation Routines
// ------------------------------------------------------------------------

/// Return internal representation of buf[offset..offset+len-1], converting '/' to '.'.
///
/// Note: the naming is the inverse of that used by JVMS 4.2 The Internal Form Of Names,
/// which defines "internal name" to be the form using "/" instead of ".".
pub fn internalize(buf: &[u8], offset: i32, len: i32) -> Vec<u8> {
    let mut translated = vec![0u8; len as usize];
    for j in 0..len as usize {
        let b = buf[offset as usize + j];
        translated[j] = if b == b'/' { b'.' } else { b };
    }
    translated
}

/// Return external representation of buf[offset..offset+len-1], converting '.' to '/'.
///
/// Note: the naming is the inverse of that used by JVMS 4.2 The Internal Form Of Names,
/// which defines "internal name" to be the form using "/" instead of ".".
pub fn externalize(buf: &[u8], offset: i32, len: i32) -> Vec<u8> {
    let mut translated = vec![0u8; len as usize];
    for j in 0..len as usize {
        let b = buf[offset as usize + j];
        translated[j] = if b == b'.' { b'/' } else { b };
    }
    translated
}

// TODO(P0+): internalize(Name) / externalize(Name) overloads are added together with
// the Name port (they are trivial wrappers around the byte[] versions above).


/// Return the external representation of `name`, converting '/' to '.'.
///
/// shared-table name, so the byte offset/length pair is read through the Name
/// accessors and copied into a fresh vector; the result never aliases the table.
///
/// # Arguments
///   - name: the external name.
///
/// # Returns
///   - the internal name bytes.
///
/// # Panics
///   - none.
///
pub fn internalize_name(name: &crate::util::name::Name) -> Vec<u8> {
    internalize(
        name.get_byte_array(),
        name.get_byte_offset(),
        name.get_byte_length(),
    )
}

/// Return the external representation of `name`, converting '.' to '/'.
///
/// `internalize_name`; it is used by the signature generator and the class-file
/// writer to emit JVMS internal names from the compiler's dotted names.
///
/// # Arguments
///   - name: the internal name.
///
/// # Returns
///   - the external name bytes.
///
/// # Panics
///   - none.
///
pub fn externalize_name(name: &crate::util::name::Name) -> Vec<u8> {
    externalize(
        name.get_byte_array(),
        name.get_byte_offset(),
        name.get_byte_length(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_numbers() {
        assert_eq!(Version::min().major(), 45);
        assert_eq!(Version::min().minor(), 3);
        assert_eq!(Version::max().major(), 61);
        assert_eq!(Version::max().minor(), 0);
        assert_eq!(Version::V61.major(), 61);
    }

    #[test]
    fn internalize_externalize() {
        assert_eq!(internalize(b"java/lang/Object", 0, 16), b"java.lang.Object");
        assert_eq!(externalize(b"java.lang.Object", 0, 16), b"java/lang/Object");
    }
}
