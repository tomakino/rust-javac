// Origin: com/sun/tools/javac/main/OptionHelper.java @ 982d53c (jdk-17.0.19-ga)

//! Helper object used by Option.process, providing access to the compilation
//! environment.
//!
//! PORTING_RULES: the Java abstract class maps to this trait; the GrumpyHelper
//! subclass keeps the upstream behavior (the value/environment accessors throw
//! IllegalArgumentException, getOwnName throws IllegalStateException), which is
//! expressed with panics because the Java exceptions are unchecked runtime
//! exceptions.
//!
//! The InvalidValueException is the ported Option.InvalidValueException: the
//! message is the localized diagnostic text (Log.localize).

use std::cell::RefCell;
use std::rc::Rc;

use crate::main::option::InvalidValueException;
use crate::main::option::Option;
use std::option::Option as StdOption;
use crate::resources::diagnostic_contract::ErrorInfo;
use crate::util::log::Log;

/// Java: public abstract class OptionHelper.
pub trait OptionHelper {
    /// Java: public abstract String get(Option option).
    fn get(&self, option: Option) -> StdOption<String>;

    /// Java: public abstract void put(String name, String value).
    fn put(&mut self, name: &str, value: &str);

    /// Java: public abstract void remove(String name).
    fn remove(&mut self, name: &str);

    /// Java: public abstract boolean handleFileManagerOption(Option option,
    /// String value).
    fn handle_file_manager_option(&mut self, option: Option, value: &str) -> bool;

    /// Java: public abstract Log getLog().
    fn get_log(&self) -> StdOption<Rc<RefCell<Log>>>;

    /// Java: public abstract String getOwnName().
    fn get_own_name(&self) -> String;

    /// Java: abstract void addFile(Path p).
    fn add_file(&mut self, path: &javac_fs::Path);

    /// Java: abstract void addClassName(String s).
    fn add_class_name(&mut self, name: &str);

    /// Java: Option.InvalidValueException newInvalidValueException(Error error)
    /// (OptionHelper.java:94-96): the localized message.
    fn new_invalid_value_exception(&self, error: ErrorInfo) -> InvalidValueException {
        let error = crate::util::jc_diagnostic::Error::from(error);
        let message = match self.get_log() {
            Some(log) => log.borrow().localize_info(error.info()),
            None => String::new(),
        };
        InvalidValueException::new(message)
    }
}

/// Java: public static class OptionHelper.GrumpyHelper.
pub struct GrumpyHelper {
    log: StdOption<Rc<RefCell<Log>>>,
}

impl GrumpyHelper {
    /// Java: public GrumpyHelper(Log log).
    pub fn new(log: StdOption<Rc<RefCell<Log>>>) -> GrumpyHelper {
        GrumpyHelper { log }
    }
}

impl OptionHelper for GrumpyHelper {
    // Java: throw new IllegalArgumentException()
    fn get(&self, _option: Option) -> StdOption<String> {
        panic!("IllegalArgumentException")
    }

    // Java: throw new IllegalArgumentException()
    fn put(&mut self, _name: &str, _value: &str) {
        panic!("IllegalArgumentException")
    }

    // Java: throw new IllegalArgumentException()
    fn remove(&mut self, _name: &str) {
        panic!("IllegalArgumentException")
    }

    // Java: throw new IllegalArgumentException()
    fn handle_file_manager_option(&mut self, _option: Option, _value: &str) -> bool {
        panic!("IllegalArgumentException")
    }

    // Java: public Log getLog() { return log; }
    fn get_log(&self) -> StdOption<Rc<RefCell<Log>>> {
        self.log.clone()
    }

    // Java: throw new IllegalStateException()
    fn get_own_name(&self) -> String {
        panic!("IllegalStateException")
    }

    // Java: throw new IllegalArgumentException(p.toString())
    fn add_file(&mut self, path: &javac_fs::Path) {
        panic!("IllegalArgumentException: {}", path.to_java_string())
    }

    // Java: throw new IllegalArgumentException(s)
    fn add_class_name(&mut self, name: &str) {
        panic!("IllegalArgumentException: {name}")
    }
}
