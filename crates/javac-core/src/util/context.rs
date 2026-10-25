// Origin: com/sun/tools/javac/util/Context.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/P3 utility-service batch.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: uncheckedCast (Context.java:201)
// End of §19 gap registration.

//! Support for an abstract context, modelled loosely after ThreadLocal
//! but using a user-provided context instead of the current thread.
//!
//! <p>Within the compiler, a single Context is used for each
//! invocation of the compiler.  The context is then used to ensure a
//! single copy of each compiler phase exists per compiler invocation.
//!
//! <p>The context can be used to assist in extending the compiler by
//! extending its components.  To do that, the extended component must
//! be registered before the base component.  We break initialization
//! cycles by (1) registering a factory for the component rather than
//! the component itself, and (2) a convention for a pattern of usage
//! in which each base component registers itself by calling an
//! instance method that is overridden in extended components.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: Java Context.Key<T> is a per-instance identity key; the Rust
//! port keys the context tables by std::any::TypeId (one slot per Rust type,
//! as decided for this port). Key<T> survives as a zero-sized type marker used
//! in signatures/constants; two Key<T> instances of the same T designate the
//! same slot (javac uses one static key per component type in practice).
//! Factory<T> is FnOnce(&mut Context) -> Rc<T>; values are stored as Rc<T> so
//! every get returns the same shared instance (Java reference semantics).
//! Java checked-state/runtime assertions (AssertionError, IllegalStateException)
//! map to panic! (PORTING_RULES section 6). Null stored values are impossible in
//! Rust; absence is modelled by Option, so Java's "null value" edge case is
//! registered as a mapping difference.
//!
//! Note: ADR 0001's CompilerContext (arena container) is a separate Rust object;
//! this Context is the component registry with the upstream semantics, and the
//! two are owned together by the compile entry point.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

/// The client creates an instance of this class for each key.
///
/// Java: Context.Key<T> (identity equality is inherited from Object; here the
/// key identity is the Rust type parameter, see the PORTING_RULES note).
pub struct Key<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
    /// Java: new Context.Key<T>()
    pub const fn new() -> Key<T> {
        Key {
            _marker: PhantomData,
        }
    }
}

impl<T> Default for Key<T> {
    fn default() -> Key<T> {
        Key::new()
    }
}

impl<T> Clone for Key<T> {
    fn clone(&self) -> Key<T> {
        *self
    }
}

impl<T> Copy for Key<T> {}

/// The client can register a factory for lazy creation of the instance.
///
/// Java: Context.Factory<T> (T make(Context c)); the factory must register its
/// result in the context (the javac convention), which get() verifies.
pub type Factory<T> = Box<dyn FnOnce(&mut Context) -> Rc<T>>;

/// The type-erased factory stored in a context slot (Java: Factory<T>).
type ContextFactoryFn = Box<dyn FnOnce(&mut Context) -> Box<dyn Any>>;

/// One slot of the underlying map: either a value or a factory.
enum ContextValue {
    /// A materialized value (a boxed Rc<T>), with the Java class name equivalent
    /// (the Rust type name) used by dump().
    Value {
        type_name: &'static str,
        value: Box<dyn Any>,
    },
    /// A preregistered factory (returns the boxed Rc<T>); make() runs at most once.
    Factory {
        type_name: &'static str,
        make: ContextFactoryFn,
    },
}

impl Default for Context {
    fn default() -> Context {
        Context::new()
    }
}

/// Support for an abstract context (Java: public class Context).
pub struct Context {
    /// The underlying map storing the data.
    /// We maintain the invariant that this table contains only mappings of the
    /// form Key<T> -> T or Key<T> -> Factory<T>.
    ///
    /// Java: protected final Map<Key<?>,Object> ht; Option models the Java
    /// checkState(map == null) guard (unreachable in normal operation, kept for
    /// fidelity and tested through the private constructor).
    ht: Option<HashMap<TypeId, ContextValue>>,
    /// The table of preregistered factories (Java: Map<Key<?>,Factory<?>> ft).
    /// Only maintained by put(Key, Factory) like upstream; get() reads ht.
    ft: Option<HashMap<TypeId, ()>>,
}

impl Context {
    /// Java: public Context() {}
    pub fn new() -> Context {
        Context {
            ht: Some(HashMap::new()),
            ft: Some(HashMap::new()),
        }
    }

    /// Set the factory for the key in this context.
    ///
    /// Java: public <T> void put(Key<T> key, Factory<T> fac)
    pub fn put_factory<T: 'static>(&mut self, factory: impl FnOnce(&mut Context) -> Rc<T> + 'static) {
        self.check_state();
        let key = TypeId::of::<T>();
        let factory: Factory<T> = Box::new(factory);
        let make: ContextFactoryFn = Box::new(move |context| {
            Box::new(factory(context)) as Box<dyn Any>
        });
        let ht = self.ht.as_mut().expect("context ht");
        let old = ht.insert(
            key,
            ContextValue::Factory {
                type_name: std::any::type_name::<T>(),
                make,
            },
        );
        // Java: if (old != null) throw new AssertionError("duplicate context value");
        if old.is_some() {
            panic!("duplicate context value");
        }
        // Java: checkState(ft); ft.put(key, fac); // cannot be duplicate if unique in ht
        let ft = self.ft.as_mut().expect("context ft");
        ft.insert(key, ());
    }

    /// Set the value for the key in this context.
    ///
    /// Java: public <T> void put(Key<T> key, T data)
    pub fn put<T: 'static>(&mut self, data: Rc<T>) {
        self.check_state();
        let key = TypeId::of::<T>();
        let ht = self.ht.as_mut().expect("context ht");
        if let Some(existing) = ht.get(&key) {
            match existing {
                // Java: "T extends Context.Factory" is checked at compile time in
                // Rust (data is Rc<T>, never a Factory wrapper); the upstream check
                // has no reachable Rust equivalent and is registered as a mapping
                // difference.
                ContextValue::Factory { .. } => {
                    // Java: old instanceof Factory -> overwrite is allowed.
                }
                ContextValue::Value { value, .. } => {
                    let same = value
                        .downcast_ref::<Rc<T>>()
                        .is_some_and(|old| Rc::ptr_eq(old, &data));
                    // Java: old != null && !(old instanceof Factory) && old != data && data != null
                    if !same {
                        panic!("duplicate context value");
                    }
                }
            }
        }
        ht.insert(
            key,
            ContextValue::Value {
                type_name: std::any::type_name::<T>(),
                value: Box::new(data),
            },
        );
    }

    /// Get the value for the key in this context.
    ///
    /// Java: public <T> T get(Key<T> key)
    pub fn get<T: 'static>(&mut self) -> Option<Rc<T>> {
        self.check_state();
        let key = TypeId::of::<T>();
        let is_factory = matches!(
            self.ht.as_ref().and_then(|ht| ht.get(&key)),
            Some(ContextValue::Factory { .. })
        );
        if is_factory {
            // Java: Object o = ht.get(key); if (o instanceof Factory<?> fac) o = fac.make(this);
            let make = match self.ht.as_mut().and_then(|ht| ht.remove(&key)) {
                Some(ContextValue::Factory { make, .. }) => make,
                _ => unreachable!("factory slot disappeared"),
            };
            let made: Box<dyn Any> = make(self);
            // Java: if (o instanceof Factory<?>) throw new AssertionError("T extends Context.Factory");
            let made_value: Rc<T> = match made.downcast::<Rc<T>>() {
                Ok(value) => *value,
                Err(_) => panic!("T extends Context.Factory"),
            };
            // Java: Assert.check(ht.get(key) == o)
            let registered = match self.ht.as_ref().and_then(|ht| ht.get(&key)) {
                Some(ContextValue::Value { value, .. }) => {
                    value.downcast_ref::<Rc<T>>().cloned()
                }
                _ => None,
            };
            match registered {
                Some(registered) if Rc::ptr_eq(&registered, &made_value) => Some(made_value),
                _ => panic!("factory did not register its result in the context"),
            }
        } else {
            let value = self.ht.as_ref().and_then(|ht| ht.get(&key))?;
            match value {
                ContextValue::Value { value, .. } => value.downcast_ref::<Rc<T>>().cloned(),
                // Invariant: only factories are materialized above; a factory here
                // would mean a concurrent mutation that cannot happen with &mut self.
                ContextValue::Factory { .. } => unreachable!("factory slot not taken"),
            }
        }
    }

    /// Java: public void dump()
    ///
    /// Java prints value.getClass() for every entry; Rust has no class object, so
    /// the stored Rust type name is printed (factory slots are materialized as a
    /// value only via get(), so dump shows them as factories).
    pub fn dump(&self) {
        if let Some(ht) = &self.ht {
            for value in ht.values() {
                match value {
                    ContextValue::Factory { type_name, .. } => {
                        eprintln!("factory<{}>", type_name)
                    }
                    ContextValue::Value { type_name, .. } => eprintln!("{}", type_name),
                }
            }
        }
    }

    /// Java: private static void checkState(Map<?,?> t) { if (t == null) throw new IllegalStateException(); }
    fn check_state(&self) {
        if self.ht.is_none() || self.ft.is_none() {
            panic!("illegal state: context table is null");
        }
    }

    /// Test-only constructor that models the Java checkState failure path.
    #[cfg(test)]
    fn with_null_state() -> Context {
        Context { ht: None, ft: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn key_marker_is_zero_sized_and_constructible() {
        let key: Key<u32> = Key::new();
        assert_eq!(std::mem::size_of_val(&key), 0);
        let _copy = key;
    }

    #[test]
    fn put_get_returns_the_same_shared_instance() {
        let mut context = Context::new();
        let value = Rc::new(42u32);
        context.put(value.clone());
        let first = context.get::<u32>().expect("stored value");
        let second = context.get::<u32>().expect("stored value");
        assert_eq!(*first, 42);
        assert!(Rc::ptr_eq(&first, &value));
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn missing_key_returns_none() {
        let mut context = Context::new();
        assert!(context.get::<u32>().is_none());
    }

    #[test]
    fn factory_is_lazy_and_memoized() {
        let mut context = Context::new();
        context.put_factory::<u32>(|context| {
            let value = Rc::new(7u32);
            context.put(value.clone());
            value
        });
        // Not materialized before the first get: the map holds the factory.
        let first = context.get::<u32>().expect("factory result");
        assert_eq!(*first, 7);
        let second = context.get::<u32>().expect("memoized result");
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn duplicate_value_put_panics() {
        let mut context = Context::new();
        context.put(Rc::new(1u32));
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.put(Rc::new(2u32));
        }));
        assert!(result.is_err());
    }

    #[test]
    fn same_value_put_is_allowed() {
        let mut context = Context::new();
        let value = Rc::new(1u32);
        context.put(value.clone());
        context.put(value.clone());
        assert_eq!(*context.get::<u32>().expect("value"), 1);
    }

    #[test]
    fn duplicate_factory_put_panics() {
        let mut context = Context::new();
        context.put_factory::<u32>(|context| {
            let value = Rc::new(1u32);
            context.put(value.clone());
            value
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.put_factory::<u32>(|context| {
                let value = Rc::new(2u32);
                context.put(value.clone());
                value
            });
        }));
        assert!(result.is_err());
    }

    #[test]
    fn put_after_factory_is_allowed_and_cancels_factory() {
        let mut context = Context::new();
        context.put_factory::<u32>(|context| {
            let value = Rc::new(1u32);
            context.put(value.clone());
            value
        });
        // Java: put(Key, T) allows overwriting a preregistered factory.
        context.put(Rc::new(3u32));
        assert_eq!(*context.get::<u32>().expect("value"), 3);
    }

    #[test]
    fn factory_must_register_its_result() {
        let mut context = Context::new();
        context.put_factory::<u32>(|_context| Rc::new(1u32));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = context.get::<u32>();
        }));
        assert!(result.is_err());
    }

    #[test]
    fn null_context_state_panics() {
        let mut context = Context::with_null_state();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = context.get::<u32>();
        }));
        assert!(result.is_err());
    }

    #[test]
    fn dump_does_not_panic() {
        let mut context = Context::new();
        context.put(Rc::new(1u32));
        context.put_factory::<u64>(|context| {
            let value = Rc::new(2u64);
            context.put(value.clone());
            value
        });
        context.dump();
    }
}
