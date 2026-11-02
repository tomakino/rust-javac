//! Access to the compiler's name table. Standard names are defined, as well as
//! methods to create new names.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! the standard-name fields live in NamesStd so that the name
//! table can hold a Weak<NamesStd> (Java: Name.Table.names creates a cycle
//! reclaimed by the GC). Names derefs to NamesStd, so Java call sites like
//! names.init keep working. Field names are kept exactly as in Java (including
//! non-snake-case ones) to keep the mapping auditable; r#yield is used because
//! yield is a Rust keyword. Construction order mirrors the Java constructor
//! exactly, because name-table indices are observable through Name.compareTo.

use std::ops::Deref;
use std::rc::Rc;

use crate::util::java_string::JavaString;
use crate::util::name::{Name, NameTableHandle};

/// The standard names of the compiler (Java: the fields of Names).
#[allow(non_snake_case)]
pub struct NamesStd {
    // operators and punctuation
    pub asterisk: Name,
    pub comma: Name,
    pub empty: Name,
    pub hyphen: Name,
    pub one: Name,
    pub slash: Name,

    // keywords
    pub _class: Name,
    pub _super: Name,
    pub _this: Name,
    pub var: Name,
    pub exports: Name,
    pub opens: Name,
    pub module: Name,
    pub provides: Name,
    pub requires: Name,
    pub to: Name,
    pub transitive: Name,
    pub uses: Name,
    pub open: Name,
    pub with: Name,
    pub r#yield: Name,

    // field and method names
    pub _name: Name,
    pub addSuppressed: Name,
    pub any: Name,
    pub append: Name,
    pub clinit: Name,
    pub clone: Name,
    pub close: Name,
    pub deserializeLambda: Name,
    pub desiredAssertionStatus: Name,
    pub equals: Name,
    pub error: Name,
    pub finalize: Name,
    pub forRemoval: Name,
    pub reflective: Name,
    pub getClass: Name,
    pub hasNext: Name,
    pub hashCode: Name,
    pub init: Name,
    pub iterator: Name,
    pub length: Name,
    pub next: Name,
    pub ordinal: Name,
    pub provider: Name,
    pub serialVersionUID: Name,
    pub toString: Name,
    pub value: Name,
    pub valueOf: Name,
    pub values: Name,
    pub readResolve: Name,
    pub readObject: Name,

    // class names
    pub java_io_Serializable: Name,
    pub java_lang_Class: Name,
    pub java_lang_Cloneable: Name,
    pub java_lang_Enum: Name,
    pub java_lang_Object: Name,

    // names of builtin classes
    pub Array: Name,
    pub Bound: Name,
    pub Method: Name,

    // package names
    pub java: Name,
    pub java_lang: Name,

    // module names
    pub java_base: Name,

    // attribute names
    pub Annotation: Name,
    pub AnnotationDefault: Name,
    pub BootstrapMethods: Name,
    pub Bridge: Name,
    pub CharacterRangeTable: Name,
    pub Code: Name,
    pub CompilationID: Name,
    pub ConstantValue: Name,
    pub Deprecated: Name,
    pub EnclosingMethod: Name,
    pub Enum: Name,
    pub Exceptions: Name,
    pub InnerClasses: Name,
    pub LineNumberTable: Name,
    pub LocalVariableTable: Name,
    pub LocalVariableTypeTable: Name,
    pub MethodParameters: Name,
    pub Module: Name,
    pub ModuleResolution: Name,
    pub NestHost: Name,
    pub NestMembers: Name,
    pub Record: Name,
    pub RuntimeInvisibleAnnotations: Name,
    pub RuntimeInvisibleParameterAnnotations: Name,
    pub RuntimeInvisibleTypeAnnotations: Name,
    pub RuntimeVisibleAnnotations: Name,
    pub RuntimeVisibleParameterAnnotations: Name,
    pub RuntimeVisibleTypeAnnotations: Name,
    pub Signature: Name,
    pub SourceFile: Name,
    pub SourceID: Name,
    pub StackMap: Name,
    pub StackMapTable: Name,
    pub Synthetic: Name,
    pub Value: Name,
    pub Varargs: Name,
    pub PermittedSubclasses: Name,

    // members of java.lang.annotation.ElementType
    pub ANNOTATION_TYPE: Name,
    pub CONSTRUCTOR: Name,
    pub FIELD: Name,
    pub LOCAL_VARIABLE: Name,
    pub METHOD: Name,
    pub MODULE: Name,
    pub PACKAGE: Name,
    pub PARAMETER: Name,
    pub TYPE: Name,
    pub TYPE_PARAMETER: Name,
    pub TYPE_USE: Name,
    pub RECORD_COMPONENT: Name,

    // members of java.lang.annotation.RetentionPolicy
    pub CLASS: Name,
    pub RUNTIME: Name,
    pub SOURCE: Name,

    // other identifiers
    pub T: Name,
    pub ex: Name,
    pub module_info: Name,
    pub package_info: Name,
    pub requireNonNull: Name,

    // lambda-related
    pub lambda: Name,
    pub metafactory: Name,
    pub altMetafactory: Name,
    pub dollarThis: Name,

    // string concat
    pub makeConcat: Name,
    pub makeConcatWithConstants: Name,

    // record related
    // members of java.lang.runtime.ObjectMethods
    pub bootstrap: Name,

    pub record: Name,
    pub non: Name,

    // serialization members, used by records too
    pub serialPersistentFields: Name,
    pub writeObject: Name,
    pub writeReplace: Name,
    pub readObjectNoData: Name,

    // sealed types
    pub permits: Name,
    pub sealed: Name,

    // pattern switches
    pub typeSwitch: Name,
    pub enumSwitch: Name,
}

/// Access to the compiler's name table.
pub struct Names {
    pub table: Rc<NameTableHandle>,
    pub std: Rc<NamesStd>,
}

impl Names {
    /// Java: public Names(Context context) { Options options = Options.instance(context); table = createTable(options); ... }
    ///
    /// the Context/Options dependency is passed as a plain flag so
    /// that P0 does not depend on the option layer; the caller passes
    /// options.isSet("useUnsharedTable").
    pub fn new(use_unshared_table: bool) -> Names {
        let table = if use_unshared_table {
            NameTableHandle::unshared()
        } else {
            NameTableHandle::shared()
        };
        let mk = |s: &str| table.from_string(&JavaString::from(s));

        // NOTE: construction order matches Names.java exactly (name indices are
        // observable through Name.compareTo).
        let std = Rc::new(NamesStd {
            // operators and punctuation
            asterisk: mk("*"),
            comma: mk(","),
            empty: mk(""),
            hyphen: mk("-"),
            one: mk("1"),
            slash: mk("/"),

            // keywords
            _class: mk("class"),
            _super: mk("super"),
            _this: mk("this"),
            var: mk("var"),
            exports: mk("exports"),
            opens: mk("opens"),
            module: mk("module"),
            provides: mk("provides"),
            requires: mk("requires"),
            to: mk("to"),
            transitive: mk("transitive"),
            uses: mk("uses"),
            open: mk("open"),
            with: mk("with"),
            r#yield: mk("yield"),

            // field and method names
            _name: mk("name"),
            addSuppressed: mk("addSuppressed"),
            any: mk("<any>"),
            append: mk("append"),
            clinit: mk("<clinit>"),
            clone: mk("clone"),
            close: mk("close"),
            deserializeLambda: mk("$deserializeLambda$"),
            desiredAssertionStatus: mk("desiredAssertionStatus"),
            equals: mk("equals"),
            error: mk("<error>"),
            finalize: mk("finalize"),
            forRemoval: mk("forRemoval"),
            reflective: mk("reflective"),
            getClass: mk("getClass"),
            hasNext: mk("hasNext"),
            hashCode: mk("hashCode"),
            init: mk("<init>"),
            iterator: mk("iterator"),
            length: mk("length"),
            next: mk("next"),
            ordinal: mk("ordinal"),
            provider: mk("provider"),
            serialVersionUID: mk("serialVersionUID"),
            toString: mk("toString"),
            value: mk("value"),
            valueOf: mk("valueOf"),
            values: mk("values"),
            readResolve: mk("readResolve"),
            readObject: mk("readObject"),
            dollarThis: mk("$this"),

            // class names
            java_io_Serializable: mk("java.io.Serializable"),
            java_lang_Class: mk("java.lang.Class"),
            java_lang_Cloneable: mk("java.lang.Cloneable"),
            java_lang_Enum: mk("java.lang.Enum"),
            java_lang_Object: mk("java.lang.Object"),

            // names of builtin classes
            Array: mk("Array"),
            Bound: mk("Bound"),
            Method: mk("Method"),

            // package names
            java: mk("java"),
            java_lang: mk("java.lang"),

            // module names
            java_base: mk("java.base"),

            // attribute names
            Annotation: mk("Annotation"),
            AnnotationDefault: mk("AnnotationDefault"),
            BootstrapMethods: mk("BootstrapMethods"),
            Bridge: mk("Bridge"),
            CharacterRangeTable: mk("CharacterRangeTable"),
            Code: mk("Code"),
            CompilationID: mk("CompilationID"),
            ConstantValue: mk("ConstantValue"),
            Deprecated: mk("Deprecated"),
            EnclosingMethod: mk("EnclosingMethod"),
            Enum: mk("Enum"),
            Exceptions: mk("Exceptions"),
            InnerClasses: mk("InnerClasses"),
            LineNumberTable: mk("LineNumberTable"),
            LocalVariableTable: mk("LocalVariableTable"),
            LocalVariableTypeTable: mk("LocalVariableTypeTable"),
            MethodParameters: mk("MethodParameters"),
            Module: mk("Module"),
            ModuleResolution: mk("ModuleResolution"),
            NestHost: mk("NestHost"),
            NestMembers: mk("NestMembers"),
            Record: mk("Record"),
            RuntimeInvisibleAnnotations: mk("RuntimeInvisibleAnnotations"),
            RuntimeInvisibleParameterAnnotations: mk("RuntimeInvisibleParameterAnnotations"),
            RuntimeInvisibleTypeAnnotations: mk("RuntimeInvisibleTypeAnnotations"),
            RuntimeVisibleAnnotations: mk("RuntimeVisibleAnnotations"),
            RuntimeVisibleParameterAnnotations: mk("RuntimeVisibleParameterAnnotations"),
            RuntimeVisibleTypeAnnotations: mk("RuntimeVisibleTypeAnnotations"),
            Signature: mk("Signature"),
            SourceFile: mk("SourceFile"),
            SourceID: mk("SourceID"),
            StackMap: mk("StackMap"),
            StackMapTable: mk("StackMapTable"),
            Synthetic: mk("Synthetic"),
            Value: mk("Value"),
            Varargs: mk("Varargs"),
            PermittedSubclasses: mk("PermittedSubclasses"),

            // members of java.lang.annotation.ElementType
            ANNOTATION_TYPE: mk("ANNOTATION_TYPE"),
            CONSTRUCTOR: mk("CONSTRUCTOR"),
            FIELD: mk("FIELD"),
            LOCAL_VARIABLE: mk("LOCAL_VARIABLE"),
            METHOD: mk("METHOD"),
            MODULE: mk("MODULE"),
            PACKAGE: mk("PACKAGE"),
            PARAMETER: mk("PARAMETER"),
            TYPE: mk("TYPE"),
            TYPE_PARAMETER: mk("TYPE_PARAMETER"),
            TYPE_USE: mk("TYPE_USE"),
            RECORD_COMPONENT: mk("RECORD_COMPONENT"),

            // members of java.lang.annotation.RetentionPolicy
            CLASS: mk("CLASS"),
            RUNTIME: mk("RUNTIME"),
            SOURCE: mk("SOURCE"),

            // other identifiers
            T: mk("T"),
            ex: mk("ex"),
            module_info: mk("module-info"),
            package_info: mk("package-info"),
            requireNonNull: mk("requireNonNull"),

            // lambda-related
            lambda: mk("lambda$"),
            metafactory: mk("metafactory"),
            altMetafactory: mk("altMetafactory"),

            // string concat
            makeConcat: mk("makeConcat"),
            makeConcatWithConstants: mk("makeConcatWithConstants"),

            bootstrap: mk("bootstrap"),
            record: mk("record"),
            non: mk("non"),

            serialPersistentFields: mk("serialPersistentFields"),
            writeObject: mk("writeObject"),
            writeReplace: mk("writeReplace"),
            readObjectNoData: mk("readObjectNoData"),

            // sealed types
            permits: mk("permits"),
            sealed: mk("sealed"),

            // pattern switches
            typeSwitch: mk("typeSwitch"),
            enumSwitch: mk("enumSwitch"),
        });
        table.set_names(&std);
        Names { table, std }
    }

    /// Java: public void dispose()
    pub fn dispose(&self) {
        self.table.dispose();
    }

    /// Java: public Name fromChars(char[] cs, int start, int len)
    pub fn from_chars(&self, cs: &[u16], start: i32, len: i32) -> Name {
        self.table.from_chars(cs, start, len)
    }

    /// Java: public Name fromString(String s)
    pub fn from_string(&self, s: &JavaString) -> Name {
        self.table.from_string(s)
    }

    /// Java: public Name fromUtf(byte[] cs)
    pub fn from_utf(&self, cs: &[u8]) -> Name {
        self.table.from_utf_all(cs)
    }

    /// Java: public Name fromUtf(byte[] cs, int start, int len)
    pub fn from_utf_range(&self, cs: &[u8], start: i32, len: i32) -> Name {
        self.table.from_utf(cs, start, len)
    }
}

impl Deref for Names {
    type Target = NamesStd;

    fn deref(&self) -> &NamesStd {
        &self.std
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_names() {
        let names = Names::new(false);
        assert_eq!(names.init.to_java_string().to_string_lossy(), "<init>");
        assert_eq!(names.clinit.to_java_string().to_string_lossy(), "<clinit>");
        assert_eq!(names._class.to_java_string().to_string_lossy(), "class");
        assert_eq!(names.java_lang_Object.to_java_string().to_string_lossy(), "java.lang.Object");
        assert_eq!(names.empty.to_java_string().to_string_lossy(), "");
        assert!(names.empty.is_empty());
        // names must be interned in the same table
        let n = names.from_string(&JavaString::from("<init>"));
        assert_eq!(n, names.init);
    }
}
