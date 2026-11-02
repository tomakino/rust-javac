//! Provides details about work that has been done by the JDK Java Compiler,
//! javac.
//!
//! Java final class -> Rust struct; nullable fields -> Option;
//! the four Java constructors are overloads of one name and become distinct
//! Rust constructors (new/new_with_source_file/new_with_unit/
//! new_with_unit_and_type) plus the private all-fields form; the Java
//! toString() -> Display (null fields print as "null", matching Java string
//! concatenation).

use crate::tree::compilation_unit_tree::CompilationUnitTree;
use javac_strings::JavaString;
use javac_tools::lang::model::element::TypeElement;
use javac_tools::tools::java_file_object::JavaFileObject;
use std::rc::Rc;

/// Provides details about work that has been done by the JDK Java Compiler, javac.
///
/// @author Jonathan Gibbons
/// @since 1.6
pub struct TaskEvent {
    kind: Kind,
    file: Option<Rc<dyn JavaFileObject>>,
    unit: Option<Rc<dyn CompilationUnitTree>>,
    clazz: Option<Rc<dyn TypeElement>>,
}

impl TaskEvent {
    /// Creates a task event for a given kind.
    /// The source file, compilation unit and type element
    /// are all set to {@code null}.
    /// @param kind the kind of the event
    pub fn new(kind: Kind) -> TaskEvent {
        TaskEvent::new_full(kind, None, None, None)
    }

    /// Creates a task event for a given kind and source file.
    /// The compilation unit and type element are both set to {@code null}.
    /// @param kind the kind of the event
    /// @param source_file the source file
    pub fn new_with_source_file(kind: Kind, source_file: Rc<dyn JavaFileObject>) -> TaskEvent {
        TaskEvent::new_full(kind, Some(source_file), None, None)
    }

    /// Creates a task event for a given kind and compilation unit.
    /// The source file is set from the compilation unit,
    /// and the type element is set to {@code null}.
    /// @param kind the kind of the event
    /// @param unit the compilation unit
    pub fn new_with_unit(kind: Kind, unit: Rc<dyn CompilationUnitTree>) -> TaskEvent {
        let file = Some(unit.get_source_file());
        TaskEvent::new_full(kind, file, Some(unit), None)
    }

    /// Creates a task event for a given kind, compilation unit
    /// and type element.
    /// The source file is set from the compilation unit.
    /// @param kind the kind of the event
    /// @param unit the compilation unit
    /// @param clazz the type element
    pub fn new_with_unit_and_type(
        kind: Kind,
        unit: Rc<dyn CompilationUnitTree>,
        clazz: Rc<dyn TypeElement>,
    ) -> TaskEvent {
        let file = Some(unit.get_source_file());
        TaskEvent::new_full(kind, file, Some(unit), Some(clazz))
    }

    /// Java: private TaskEvent(Kind, JavaFileObject, CompilationUnitTree, TypeElement).
    fn new_full(
        kind: Kind,
        file: Option<Rc<dyn JavaFileObject>>,
        unit: Option<Rc<dyn CompilationUnitTree>>,
        clazz: Option<Rc<dyn TypeElement>>,
    ) -> TaskEvent {
        TaskEvent {
            kind,
            file,
            unit,
            clazz,
        }
    }

    /// Returns the kind for this event.
    /// @return the kind
    pub fn get_kind(&self) -> Kind {
        self.kind
    }

    /// Returns the source file for this event.
    /// May be {@code null}.
    /// @return the source file
    pub fn get_source_file(&self) -> Option<Rc<dyn JavaFileObject>> {
        self.file.clone()
    }

    /// Returns the compilation unit for this event.
    /// May be {@code null}.
    /// @return the compilation unit
    pub fn get_compilation_unit(&self) -> Option<Rc<dyn CompilationUnitTree>> {
        self.unit.clone()
    }

    /// Returns the type element for this event.
    /// May be {@code null}.
    /// @return the type element
    pub fn get_type_element(&self) -> Option<Rc<dyn TypeElement>> {
        self.clazz.clone()
    }
}

impl std::fmt::Display for TaskEvent {
    /// Java: toString() -> "TaskEvent[" + kind + "," + file + "," + clazz + "]".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let file = match &self.file {
            Some(file) => file.get_name().to_string_lossy(),
            None => "null".to_string(),
        };
        let clazz = match &self.clazz {
            Some(clazz) => clazz.to_string(),
            None => "null".to_string(),
        };
        write!(f, "TaskEvent[{},{},{}]", self.kind, file, clazz)
    }
}

/// Kind of task event.
/// @since 1.6
///
/// Java nested enum TaskEvent.Kind -> task_event::Kind.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// For events related to the parsing of a file.
    PARSE,
    /// For events relating to elements being entered.
    ENTER,
    /// For events relating to elements being analyzed for errors.
    ANALYZE,
    /// For events relating to class files being generated.
    GENERATE,
    /// For events relating to overall annotation processing.
    ANNOTATION_PROCESSING,
    /// For events relating to an individual annotation processing round.
    ANNOTATION_PROCESSING_ROUND,
    /// Sent before parsing first source file, and after writing the last output
    /// file.  This event is not sent when using {@link JavacTask#parse()},
    /// {@link JavacTask#analyze()} or {@link JavacTask#generate()}.
    ///
    /// @since 9
    COMPILATION,
}

impl Kind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Kind::PARSE => "PARSE",
            Kind::ENTER => "ENTER",
            Kind::ANALYZE => "ANALYZE",
            Kind::GENERATE => "GENERATE",
            Kind::ANNOTATION_PROCESSING => "ANNOTATION_PROCESSING",
            Kind::ANNOTATION_PROCESSING_ROUND => "ANNOTATION_PROCESSING_ROUND",
            Kind::COMPILATION => "COMPILATION",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [Kind; 7] {
        [
            Kind::PARSE,
            Kind::ENTER,
            Kind::ANALYZE,
            Kind::GENERATE,
            Kind::ANNOTATION_PROCESSING,
            Kind::ANNOTATION_PROCESSING_ROUND,
            Kind::COMPILATION,
        ]
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, TaskEvent};
    use crate::util::test_support::UnitTree;
    use std::rc::Rc;

    #[test]
    fn kind_enum_is_complete_and_ordered() {
        let values = Kind::values();
        assert_eq!(values.len(), 7);
        assert_eq!(values[0], Kind::PARSE);
        assert_eq!(values[6], Kind::COMPILATION);
        assert_eq!(
            Kind::ANNOTATION_PROCESSING_ROUND.name().to_string_lossy(),
            "ANNOTATION_PROCESSING_ROUND"
        );
    }

    #[test]
    fn constructors_and_getters() {
        let event = TaskEvent::new(Kind::PARSE);
        assert_eq!(event.get_kind(), Kind::PARSE);
        assert!(event.get_source_file().is_none());
        assert!(event.get_compilation_unit().is_none());
        assert!(event.get_type_element().is_none());
        assert_eq!(event.to_string(), "TaskEvent[PARSE,null,null]");

        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTree);
        let event = TaskEvent::new_with_unit(Kind::ENTER, unit);
        assert!(event.get_source_file().is_some());
        assert!(event.get_compilation_unit().is_some());
        assert_eq!(event.to_string(), "TaskEvent[ENTER,Test.java,null]");
    }
}
