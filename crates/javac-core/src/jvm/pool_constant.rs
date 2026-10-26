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

use crate::ids::{SymbolId, TypeId};

/// Gen/PoolWriter batch).
#[derive(Clone, Debug, PartialEq)]
pub enum PoolKey {
    Integer(i32),
    Float(u32),
    Long(i64),
    Double(u64),
    String(JavaString),
    /// A Utf8 name pool key.
    Utf8(Name),
    /// A class-type loadable constant (Java: ClassType pool key).
    Class(TypeId),
    /// A method-handle loadable constant.
    MethodHandle(SymbolId),
    /// A dynamic (invokedynamic/constant dynamic) loadable constant.
    Dynamic(SymbolId),
    /// A pool key wrapping another unique type pool key.
    Type(TypeId),
    /// A pool key wrapping a symbol identity.
    Symbol(SymbolId),
    /// A pair pool key (Java: Pair).
    Pair(Box<PoolKey>, Box<PoolKey>),
    /// A name-and-type pool key.
    NameAndType(Name, TypeId),
}

use crate::util::name::Name;

pub trait PoolConstant {
    fn pool_key(&self) -> PoolKey;

    fn pool_tag(&self) -> i32;
}

pub trait LoadableConstant: PoolConstant {}

#[derive(Clone, Debug, PartialEq)]
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

/// The element type of a loadable constant array.
///
/// The upstream LoadableConstant[] holds interface references; the ported code
/// stores the concrete producers in this enum so the array can live inside a
/// symbol (trait objects cannot be cloned into the arena) while keeping every
/// Java producer (BasicConstant, ClassType, MethodHandleSymbol, dynamic
/// symbols) individually recognizable.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadableConstantRef {
    /// A basic constant (int/float/long/double/String).
    Basic(BasicConstant),
    /// A class type (Java: ClassType implements LoadableConstant).
    Class(TypeId),
    /// A method handle symbol.
    MethodHandle(SymbolId),
    /// A dynamic symbol (invokedynamic/constant dynamic).
    Dynamic(SymbolId),
}

impl PoolConstant for LoadableConstantRef {
    fn pool_key(&self) -> PoolKey {
        match self {
            LoadableConstantRef::Basic(basic) => basic.pool_key(),
            LoadableConstantRef::Class(ty) => PoolKey::Class(*ty),
            LoadableConstantRef::MethodHandle(sym) => PoolKey::MethodHandle(*sym),
            LoadableConstantRef::Dynamic(sym) => PoolKey::Dynamic(*sym),
        }
    }

    fn pool_tag(&self) -> i32 {
        match self {
            LoadableConstantRef::Basic(basic) => basic.pool_tag(),
            LoadableConstantRef::Class(ty) => {
                let _ = ty;
                crate::jvm::class_file::CONSTANT_CLASS
            }
            LoadableConstantRef::MethodHandle(sym) => {
                let _ = sym;
                crate::jvm::class_file::CONSTANT_METHODHANDLE
            }
            LoadableConstantRef::Dynamic(sym) => {
                let _ = sym;
                crate::jvm::class_file::CONSTANT_INVOKEDYNAMIC
            }
        }
    }
}

impl LoadableConstant for LoadableConstantRef {}

/// A name-and-type constant pool entry.
#[derive(Clone, Debug, PartialEq)]
pub struct NameAndType {
    /// The member name.
    pub name: Name,
    /// The member type.
    pub type_: TypeId,
}

impl NameAndType {
    /// Create a name-and-type constant.
    pub fn new(name: Name, type_: TypeId) -> NameAndType {
        NameAndType { name, type_ }
    }
}

impl PoolConstant for NameAndType {
    fn pool_key(&self) -> PoolKey {
        PoolKey::NameAndType(self.name.clone(), self.type_)
    }

    fn pool_tag(&self) -> i32 {
        crate::jvm::class_file::CONSTANT_NAMEANDTYPE
    }
}

/// The contract of a dynamic pool constant (invokedynamic or constant dynamic).
///
/// The upstream Dynamic interface exposes the dynamic type, the bootstrap
/// method and the static argument list, plus a bootstrap-method key used to
/// deduplicate BootstrapMethods attribute entries. The ported dynamic symbols
/// live in the symbol arena, so the trait is implemented here for the shared
/// helper views rather than for the arena values themselves.
pub trait Dynamic: PoolConstant {
    /// Return the dynamic type of this constant.
    fn dynamic_type(&self) -> PoolKey;
    /// Return the bootstrap method handle.
    fn bootstrap_method(&self) -> Option<SymbolId>;
    /// Return the static argument list.
    fn static_args(&self) -> Vec<LoadableConstantRef>;
}

/// A bootstrap method key used to deduplicate BootstrapMethods entries.
#[derive(Clone, Debug, PartialEq)]
pub struct BsmKey {
    /// The bootstrap method constant.
    pub bsm: Option<SymbolId>,
    /// The static argument list.
    pub static_args: Vec<LoadableConstantRef>,
    /// The pool key of the bootstrap method.
    pub bsm_key: PoolKey,
    /// The pool keys of the static arguments.
    pub static_arg_keys: Vec<PoolKey>,
}

impl BsmKey {
    /// Build the bootstrap method key for a dynamic pool constant.
    pub fn new(dynamic: &dyn Dynamic) -> BsmKey {
        let bsm = dynamic.bootstrap_method();
        let static_args = dynamic.static_args();
        let bsm_key = match bsm {
            Some(sym) => PoolKey::MethodHandle(sym),
            None => PoolKey::Symbol(crate::ids::SymbolId::new(0)),
        };
        let static_arg_keys = static_args
            .iter()
            .map(|arg| arg.pool_key())
            .collect::<Vec<_>>();
        BsmKey {
            bsm,
            static_args,
            bsm_key,
            static_arg_keys,
        }
    }
}
