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

use javac_strings::JavaString;

/// Gen/PoolWriter batch).
#[derive(Clone, Debug, PartialEq)]
pub enum PoolKey {
    Integer(i32),
    Float(u32),
    Long(i64),
    Double(u64),
    String(JavaString),
}

pub trait PoolConstant {
    fn pool_key(&self) -> PoolKey;

    fn pool_tag(&self) -> i32;
}

pub trait LoadableConstant: PoolConstant {}

#[derive(Clone, Debug)]
pub struct BasicConstant {
    pub tag: i32,
    pub data: PoolKey,
}

impl BasicConstant {
    fn new(tag: i32, data: PoolKey) -> BasicConstant {
        BasicConstant { tag, data }
    }

/// Create a pool constant describing the given `int` value.
///
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
    pub fn int(i: i32) -> BasicConstant {
        BasicConstant::new(crate::jvm::class_file::CONSTANT_INTEGER, PoolKey::Integer(i))
    }

/// Create a pool constant describing the given `float` value.
///
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
    pub fn float(f: f32) -> BasicConstant {
        BasicConstant::new(
            crate::jvm::class_file::CONSTANT_FLOAT,
            PoolKey::Float(f.to_bits()),
        )
    }

/// Create a pool constant describing the given `long` value.
///
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
    pub fn long(l: i64) -> BasicConstant {
        BasicConstant::new(crate::jvm::class_file::CONSTANT_LONG, PoolKey::Long(l))
    }

/// Create a pool constant describing the given `double` value.
///
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
    pub fn double(d: f64) -> BasicConstant {
        BasicConstant::new(
            crate::jvm::class_file::CONSTANT_DOUBLE,
            PoolKey::Double(d.to_bits()),
        )
    }

/// Create a pool constant describing the given `String` value.
///
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
    pub fn string(s: JavaString) -> BasicConstant {
        BasicConstant::new(crate::jvm::class_file::CONSTANT_STRING, PoolKey::String(s))
    }
}

impl PoolConstant for BasicConstant {
/// Return the constant-pool entry key of this basic constant.
///
/// the constant itself; BasicConstant overrides it with the boxed payload so that
/// structurally equal values deduplicate in the constant pool.
///
/// # Returns
///   - the payload pool key.
///
/// # Panics
///   - none.
///
    fn pool_key(&self) -> PoolKey {
        self.data.clone()
    }

/// Return the JVMS constant-pool tag of this basic constant.
///
/// entry (CONSTANT_Integer, CONSTANT_Float, CONSTANT_Long, CONSTANT_Double or
/// CONSTANT_String for the LoadableConstant.BasicConstant family).
///
/// # Returns
///   - the constant-pool tag.
///
/// # Panics
///   - none.
///
    fn pool_tag(&self) -> i32 {
        self.tag
    }
}

impl LoadableConstant for BasicConstant {}
