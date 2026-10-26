// Origin: com/sun/tools/javac/jvm/PoolConstant.java @ 982d53c (jdk-17.0.19-ga)

//! This interface models all javac entities that can be used to represent
//! constant pool entries. A pool constant entity must (i) be associated with a
//! constant pool entry tag and (ii) have a function which generates a key for
//! the desired pool entry (so as to avoid duplicate entries when writing the
//! constant pool).
//!
//! PORTING_RULES: the ported subset covers the LoadableConstant hierarchy used
//! by the Lower pass; the Dynamic/BsmKey/NameAndType members of the upstream
//! file are registered for the Gen/PoolWriter batch (they need the UniqueType
//! adapter and the bootstrap-method pool machinery).
//!
//! Upstream: PoolConstant.java:33-220.

use javac_strings::JavaString;

/// Java: the Object pool key values of the basic constants
/// (PoolConstant.java:60-127; the full key hierarchy is registered with the
/// Gen/PoolWriter batch).
#[derive(Clone, Debug, PartialEq)]
pub enum PoolKey {
    /// Java: Integer data
    Integer(i32),
    /// Java: Float data
    Float(u32),
    /// Java: Long data
    Long(i64),
    /// Java: Double data
    Double(u64),
    /// Java: String data
    String(JavaString),
}

/// Java: public interface PoolConstant (PoolConstant.java:33-38).
pub trait PoolConstant {
    /// Java: default Object poolKey(Types types) { return this; }
    /// (PoolConstant.java:43-45).
    fn pool_key(&self) -> PoolKey;

    /// Java: int poolTag() (PoolConstant.java:50).
    fn pool_tag(&self) -> i32;
}

/// Java: interface PoolConstant.LoadableConstant (PoolConstant.java:60-127).
pub trait LoadableConstant: PoolConstant {}

/// Java: class PoolConstant.LoadableConstant.BasicConstant
/// (PoolConstant.java:105-127).
#[derive(Clone, Debug)]
pub struct BasicConstant {
    /// Java: int tag
    pub tag: i32,
    /// Java: Object data (the PoolKey payload)
    pub data: PoolKey,
}

impl BasicConstant {
    /// Java: private BasicConstant(int tag, Object data) (PoolConstant.java:110-113).
    fn new(tag: i32, data: PoolKey) -> BasicConstant {
        BasicConstant { tag, data }
    }

/// Create a pool constant describing the given `int` value.
///
/// Java: static LoadableConstant Int(int i). The constant is a BasicConstant with
/// tag CONSTANT_Integer whose pool key is the boxed integer; two equal integers
/// therefore share one constant-pool entry.
///
/// # Arguments
///   - i: the integer value.
///
/// # Returns
///   - the basic loadable constant.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:65-67.
    pub fn int(i: i32) -> BasicConstant {
        BasicConstant::new(crate::jvm::class_file::CONSTANT_INTEGER, PoolKey::Integer(i))
    }

/// Create a pool constant describing the given `float` value.
///
/// Java: static LoadableConstant Float(float f). The value is stored as its raw
/// bit pattern so that pool-key equality follows Java Float.equals semantics
/// (NaN == NaN and +0.0 != -0.0 in the constant pool).
///
/// # Arguments
///   - f: the float value.
///
/// # Returns
///   - the basic loadable constant.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:72-74.
    pub fn float(f: f32) -> BasicConstant {
        BasicConstant::new(
            crate::jvm::class_file::CONSTANT_FLOAT,
            PoolKey::Float(f.to_bits()),
        )
    }

/// Create a pool constant describing the given `long` value.
///
/// Java: static LoadableConstant Long(long l). The constant carries
/// tag CONSTANT_Long; JVMS 4.4.5 counts such entries as two slots in the pool,
/// which is handled by the pool writer, not by this key.
///
/// # Arguments
///   - l: the long value.
///
/// # Returns
///   - the basic loadable constant.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:79-81.
    pub fn long(l: i64) -> BasicConstant {
        BasicConstant::new(crate::jvm::class_file::CONSTANT_LONG, PoolKey::Long(l))
    }

/// Create a pool constant describing the given `double` value.
///
/// Java: static LoadableConstant Double(double d). As for Float, the raw bits are
/// used for the pool key so that the constant-pool deduplication matches Java
/// Double.equals semantics.
///
/// # Arguments
///   - d: the double value.
///
/// # Returns
///   - the basic loadable constant.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:86-88.
    pub fn double(d: f64) -> BasicConstant {
        BasicConstant::new(
            crate::jvm::class_file::CONSTANT_DOUBLE,
            PoolKey::Double(d.to_bits()),
        )
    }

/// Create a pool constant describing the given `String` value.
///
/// Java: static LoadableConstant String(String s). The constant uses
/// tag CONSTANT_String and a string pool key; the same text used by several
/// instructions is merged into a single constant-pool entry.
///
/// # Arguments
///   - s: the string value.
///
/// # Returns
///   - the basic loadable constant.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:93-95.
    pub fn string(s: JavaString) -> BasicConstant {
        BasicConstant::new(crate::jvm::class_file::CONSTANT_STRING, PoolKey::String(s))
    }
}

impl PoolConstant for BasicConstant {
/// Return the constant-pool entry key of this basic constant.
///
/// Java: public Object poolKey(Types types). The upstream default would return
/// the constant itself; BasicConstant overrides it with the boxed payload so that
/// structurally equal values deduplicate in the constant pool.
///
/// # Returns
///   - the payload pool key.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:121-124.
    fn pool_key(&self) -> PoolKey {
        self.data.clone()
    }

/// Return the JVMS constant-pool tag of this basic constant.
///
/// Java: public int poolTag(). The tag determines how the pool writer encodes the
/// entry (CONSTANT_Integer, CONSTANT_Float, CONSTANT_Long, CONSTANT_Double or
/// CONSTANT_String for the LoadableConstant.BasicConstant family).
///
/// # Returns
///   - the constant-pool tag.
///
/// # Panics
///   - none.
///
/// Upstream: PoolConstant.java:116-119.
    fn pool_tag(&self) -> i32 {
        self.tag
    }
}

impl LoadableConstant for BasicConstant {}
