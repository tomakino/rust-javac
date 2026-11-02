//! A table of all command-line options.
//! If an option has an argument, the option name is mapped to the argument.
//! If a set option has no argument, it is mapped to itself.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//!
//! * Java objects are shared mutable references; the Rust Options table is shared
//!   through Rc<Options> (Context-cached) and every method takes &self, so the
//!   mutable state lives in RefCell.  This is a registered interior-mutability
//!   allowance (Porting rules.11): the borrow windows are the individual table operations,
//!   they never cross calls and there is no object graph built out of it.  A
//!   listener that mutates the listener list from inside notifyListeners would
//!   panic (RefCell re-borrow); upstream javac never does that (registered).
//! * Java LinkedHashMap<String,String> -> RefCell<Vec<(String, String)>> keeping
//!   the insertion order (LinkedHashMap iteration order, Porting rules); put of an existing
//!   key keeps its position, remove deletes the entry, keySet returns a snapshot in
//!   insertion order, putAll updates/inserts in the argument's order.
//! * get(String) returns a copy (Rust cannot return a reference out of a RefCell);
//!   Java returns the stored String reference — registered shape difference.
//! * Java List<Runnable> listeners (prepend + iterate from the head) ->
//!   RefCell<Vec<Box<dyn FnMut()>>> with insert(0) and forward iteration: the LIFO
//!   notification order is the same.
//! * Java protected Options(Context) -> the private new_table() used by
//!   instance(Context); the Context caches the Rc<Options> (Context keys by Rust
//!   type, see util/context.rs).
//! * Option arguments: Option from main::option, using Option::primary_name().
//!   Rust's std Option is aliased as StdOption in this file because the upstream
//!   class name Option shadows it (Porting rules name fidelity is kept).
//! * Map values are never null upstream (absence is what get() returns as null);
//!   Rust models absence with None, so a null value cannot be stored (Porting rules note).
//! * REGISTERED: the full Arguments/CommandLine table is not ported; Options
//!   stores the values supplied by the compiler wiring. isLintSet and the
//!   -Xlint parsing needed by the ported Lint/Preview callers are implemented.

use std::cell::RefCell;
use std::option::Option as StdOption;
use std::rc::Rc;

use crate::main::option::Option;
use crate::util::context::Context;

/// A table of all command-line options.
pub struct Options {
    /// Java: private LinkedHashMap<String,String> values.
    values: RefCell<Vec<(String, String)>>,
    /// Java: private List<Runnable> listeners = List.nil().
    listeners: RefCell<Vec<Box<dyn FnMut()>>>,
}

impl Options {
    /// Get the Options instance for this context.
    ///
    /// Java:
    /// <pre>
    /// public static Options instance(Context context) {
    ///     Options instance = context.get(optionsKey);
    ///     if (instance == null)
    ///         instance = new Options(context);
    ///     return instance;
    /// }
    /// </pre>
    pub fn instance(context: &mut Context) -> Rc<Options> {
        if let Some(instance) = context.get::<Options>() {
            return instance;
        }
        let options = Rc::new(Options::new_table());
        context.put(options.clone());
        options
    }

    /// Java: protected Options(Context context) { values = new LinkedHashMap<>();
    ///       context.put(optionsKey, this); }
    ///
    /// the context registration is done by instance(), because a
    /// Java constructor can register "this" while Rust needs the Rc first.
    fn new_table() -> Options {
        Options {
            values: RefCell::new(Vec::new()),
            listeners: RefCell::new(Vec::new()),
        }
    }

    /// Get the value for an undocumented option.
    ///
    /// returns a copy of the stored value (see the header).
    pub fn get(&self, name: &str) -> StdOption<String> {
        self.values
            .borrow()
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    /// Get the value for an option.
    ///
    /// Java: String get(Option option) — overloads use distinct Rust names (Porting rules).
    pub fn get_option(&self, option: Option) -> StdOption<String> {
        self.get(option.primary_name())
    }

    /// Java: Lint.isEnabled(LintCategory) — REGISTERED: the full Lint option set
    /// is not ported; the -Xlint options needed by the ported callers are parsed
    /// here with the upstream Lint construction rules: the global default is
    /// -Xlint or -Xlint:all (all enabled), -Xlint:none (all disabled), or the
    /// on-by-default set (CLASSFILE is not in it); the per-category option
    /// Xlint:<category> adds and Xlint:-<category> removes (add wins). The
    /// on-by-default set for other categories is registered as a Port adapter
    /// (only CLASSFILE is queried in D5).
    pub fn lint_enabled(&self, category: &str) -> bool {
        let values = self.values.borrow();
        let mut items: Vec<String> = Vec::new();
        for (key, _) in values.iter() {
            if key == "Xlint" {
                items.push("all".to_string());
            } else if let Some(spec) = key.strip_prefix("Xlint:") {
                for item in spec.split(',') {
                    items.push(item.to_string());
                }
            }
        }
        let has = |item: &str| items.iter().any(|value| value == item);
        // Upstream Lint: -Xlint / -Xlint:all wins over -Xlint:none; otherwise
        // the on-by-default set (CLASSFILE is not enabled by default).
        // -Xlint / -Xlint:all wins over -Xlint:none; the on-by-default set
        // does not contain CLASSFILE (registered Port adapter for the set).
        let mut enabled = has("all");
        // Per-category overrides: Xlint:<category> adds, Xlint:-<category>
        // removes; the add wins when both are present in the option list.
        let add = format!("Xlint:{category}");
        let remove = format!("-{category}");
        let _ = (&add, &remove);
        if items.iter().any(|item| item == category) {
            enabled = true;
        } else if items.iter().any(|item| item == &format!("-{category}")) {
            enabled = false;
        }
        enabled
    }
    /// Get the boolean value for an option, patterned after Boolean.getBoolean,
    /// essentially will return true, iff the value exists and is set to "true".
    pub fn get_boolean(&self, name: &str) -> bool {
        self.get_boolean_or(name, false)
    }

    /// Get the boolean with a default value if the option is not set.
    ///
    /// Java: getBoolean(String, boolean) — overload renamed (Porting rules).
    pub fn get_boolean_or(&self, name: &str, default_value: bool) -> bool {
        match self.get(name) {
            // Java: Boolean.parseBoolean(value) = "true".equalsIgnoreCase(value)
            None => default_value,
            Some(value) => value.eq_ignore_ascii_case("true"),
        }
    }

    /// Check if the value for an undocumented option has been set.
    pub fn is_set(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Check if the value for an option has been set.
    ///
    /// Java: boolean isSet(Option option) — overload renamed (Porting rules).
    pub fn is_set_option(&self, option: Option) -> bool {
        self.is_set(option.primary_name())
    }

    /// Check if the value for a choice option has been set to a specific value.
    ///
    /// Java: boolean isSet(Option option, String value).
    pub fn is_set_choice(&self, option: Option, value: &str) -> bool {
        self.is_set(&format!("{}{}", option.primary_name(), value))
    }

    /// Check if the value for a lint option has been explicitly set, either with -Xlint:opt
    /// or if all lint options have enabled and this one not disabled with -Xlint:-opt.
    pub fn is_lint_set(&self, s: &str) -> bool {
        // return true if either the specific option is enabled, or
        // they are all enabled without the specific one being
        // disabled
        self.is_set_choice(Option::XLINT_CUSTOM, s)
            || ((self.is_set_option(Option::XLINT)
                || self.is_set_choice(Option::XLINT_CUSTOM, "all"))
                && self.is_unset_choice(Option::XLINT_CUSTOM, &format!("-{s}")))
    }

    /// Check if the value for an undocumented option has not been set.
    pub fn is_unset(&self, name: &str) -> bool {
        self.get(name).is_none()
    }

    /// Check if the value for an option has not been set.
    ///
    /// Java: boolean isUnset(Option option) — overload renamed (Porting rules).
    pub fn is_unset_option(&self, option: Option) -> bool {
        self.is_unset(option.primary_name())
    }

    /// Check if the value for a choice option has not been set to a specific value.
    ///
    /// Java: boolean isUnset(Option option, String value).
    pub fn is_unset_choice(&self, option: Option, value: &str) -> bool {
        self.is_unset(&format!("{}{}", option.primary_name(), value))
    }

    /// Java: public void put(String name, String value).
    pub fn put(&self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        let mut values = self.values.borrow_mut();
        if let Some(entry) = values.iter_mut().find(|(key, _)| *key == name) {
            // LinkedHashMap.put on an existing key keeps the insertion position.
            entry.1 = value;
        } else {
            values.push((name, value));
        }
    }

    /// Java: public void put(Option option, String value).
    pub fn put_option(&self, option: Option, value: impl Into<String>) {
        self.put(option.primary_name(), value);
    }

    /// Java: public void putAll(Options options).
    pub fn put_all(&self, options: &Options) {
        // Snapshot first: putAll(this) must not hold two borrows (upstream allows it).
        let entries = options.values.borrow().clone();
        for (name, value) in entries {
            self.put(name, value);
        }
    }

    /// Java: public void remove(String name).
    pub fn remove(&self, name: &str) {
        self.values.borrow_mut().retain(|(key, _)| key != name);
    }

    /// Java: public Set<String> keySet().
    ///
    /// Java Set -> Vec snapshot (Porting rules), in insertion (LinkedHashMap) order.
    pub fn key_set(&self) -> Vec<String> {
        self.values
            .borrow()
            .iter()
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Java: public int size().
    pub fn size(&self) -> usize {
        self.values.borrow().len()
    }

    // light-weight notification mechanism

    /// Java: public void addListener(Runnable listener) { listeners = listeners.prepend(listener); }
    pub fn add_listener(&self, listener: Box<dyn FnMut()>) {
        self.listeners.borrow_mut().insert(0, listener);
    }

    /// Java: public void notifyListeners() { for (Runnable r: listeners) r.run(); }
    pub fn notify_listeners(&self) {
        for listener in self.listeners.borrow_mut().iter_mut() {
            listener();
        }
    }

    /// Java: public void clear() { values.clear(); listeners = List.nil(); }
    pub fn clear(&self) {
        self.values.borrow_mut().clear();
        self.listeners.borrow_mut().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::Options;
    use crate::main::option::Option;
    use crate::util::context::Context;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn instance_is_cached_in_the_context() {
        let mut context = Context::new();
        let first = Options::instance(&mut context);
        let second = Options::instance(&mut context);
        assert!(Rc::ptr_eq(&first, &second));
        let mut other_context = Context::new();
        let other = Options::instance(&mut other_context);
        assert!(!Rc::ptr_eq(&first, &other));
    }

    #[test]
    fn put_get_and_option_overloads_use_the_primary_name() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        options.put_option(Option::G, "-g");
        assert_eq!(options.get("-g"), Some("-g".to_string()));
        assert_eq!(options.get_option(Option::G), Some("-g".to_string()));
        assert_eq!(options.get_option(Option::CLASS_PATH), None);
        options.put("custom", "value");
        assert_eq!(options.get("custom"), Some("value".to_string()));
        assert_eq!(options.size(), 2);
    }

    #[test]
    fn get_boolean_follows_boolean_parse_boolean() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        assert!(!options.get_boolean("flag"));
        assert!(options.get_boolean_or("flag", true));
        options.put("flag", "true");
        assert!(options.get_boolean("flag"));
        // The default is only used when the option is absent.
        assert!(options.get_boolean_or("flag", false));
        options.put("flag", "TRUE");
        assert!(options.get_boolean("flag"));
        options.put("flag", "yes");
        assert!(!options.get_boolean("flag"));
        assert!(!options.get_boolean_or("flag", true));
    }

    #[test]
    fn set_and_unset_overloads() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        assert!(options.is_unset("x"));
        assert!(options.is_unset_option(Option::G));
        options.put("x", "1");
        options.put_option(Option::G, "-g");
        assert!(options.is_set("x"));
        assert!(options.is_set_option(Option::G));
        assert!(options.is_unset_option(Option::XLINT));

        options.put("-Xlint:deprecation", "deprecation");
        assert!(options.is_set_choice(Option::XLINT_CUSTOM, "deprecation"));
        assert!(options.is_unset_choice(Option::XLINT_CUSTOM, "rawtypes"));
        assert!(options.is_unset_choice(Option::XLINT_CUSTOM, "-rawtypes"));
    }

    #[test]
    fn is_lint_set_matches_the_upstream_formula() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        assert!(!options.is_lint_set("rawtypes"));

        // -Xlint:rawtypes enables just that one
        options.put("-Xlint:rawtypes", "rawtypes");
        assert!(options.is_lint_set("rawtypes"));
        assert!(!options.is_lint_set("unchecked"));

        // -Xlint:all enables everything not explicitly disabled
        options.put("-Xlint:all", "all");
        assert!(options.is_lint_set("unchecked"));
        options.put("-Xlint:-unchecked", "-unchecked");
        assert!(!options.is_lint_set("unchecked"));
        assert!(options.is_lint_set("rawtypes"));

        // -Xlint alone enables everything not explicitly disabled
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        options.put_option(Option::XLINT, "-Xlint");
        assert!(options.is_lint_set("deprecation"));
        options.put("-Xlint:-deprecation", "-deprecation");
        assert!(!options.is_lint_set("deprecation"));
    }

    #[test]
    fn put_all_remove_size_and_key_set_keep_insertion_order() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        let mut other_context = Context::new();
        let other = Options::instance(&mut other_context);
        other.put("a", "1");
        other.put("b", "2");
        options.put("c", "3");
        options.put_all(&other);
        assert_eq!(options.key_set(), vec!["c", "a", "b"]);
        assert_eq!(options.size(), 3);
        options.remove("a");
        assert_eq!(options.key_set(), vec!["c", "b"]);
        assert_eq!(options.size(), 2);
    }

    #[test]
    fn updating_an_existing_key_keeps_its_position() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        options.put("a", "1");
        options.put("b", "2");
        options.put("a", "updated");
        assert_eq!(options.key_set(), vec!["a", "b"]);
        assert_eq!(options.get("a"), Some("updated".to_string()));
    }

    #[test]
    fn put_all_of_itself_is_safe() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        options.put("a", "1");
        options.put_all(&options);
        assert_eq!(options.get("a"), Some("1".to_string()));
        assert_eq!(options.size(), 1);
    }

    #[test]
    fn listeners_are_notified_in_prepend_order_and_clear_drops_them() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        let log = Rc::new(RefCell::new(Vec::<u32>::new()));
        let first = log.clone();
        options.add_listener(Box::new(move || first.borrow_mut().push(1)));
        let second = log.clone();
        options.add_listener(Box::new(move || second.borrow_mut().push(2)));
        options.notify_listeners();
        // Java prepends each listener, so the most recent runs first.
        assert_eq!(*log.borrow(), vec![2, 1]);

        options.clear();
        options.notify_listeners();
        assert_eq!(*log.borrow(), vec![2, 1]);
        assert_eq!(options.size(), 0);
    }
}

#[cfg(test)]
mod d5_lint_tests {
    use super::Options;
    use crate::util::context::Context;

    fn lint(keys: &[&str]) -> bool {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        for key in keys {
            options.put(*key, "");
        }
        options.lint_enabled("classfile")
    }

    #[test]
    fn lint_enabled_matches_upstream_lint_rules() {
        assert!(!lint(&[]), "CLASSFILE is not on by default");
        assert!(lint(&["Xlint"]), "-Xlint enables all categories");
        assert!(lint(&["Xlint:all"]), "-Xlint:all enables all categories");
        assert!(!lint(&["Xlint:none"]), "-Xlint:none disables all");
        assert!(!lint(&["Xlint:all", "Xlint:-classfile"]), "negative removes");
        assert!(lint(&["Xlint:none", "Xlint:classfile"]), "positive overrides none");
        assert!(lint(&["Xlint:all,classfile"]), "comma list (all,classfile)");
        assert!(lint(&["Xlint:all,none"]), "all wins over none");
        assert!(lint(&["Xlint:classfile,-classfile"]), "add wins over remove");
    }
}