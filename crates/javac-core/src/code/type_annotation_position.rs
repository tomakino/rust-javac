//! Type annotation positions and type paths, as read from classfiles and as
//! used by the Annotate/Code layers.
//!
//! JCLambda -> Option<TreeId> (tree arena, ADR 0001);
//! java.util.List<Integer> for the type path binary form -> List<i32>;
//! Assert.* checks -> assert!/panic (same invariant violations).

use crate::code::target_type::TargetType;
use crate::ids::TreeId;
use crate::util::list::List;

/// Java: public enum TypePathEntryKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypePathEntryKind {
    ARRAY,
    INNER_TYPE,
    WILDCARD,
    TYPE_ARGUMENT,
}

impl TypePathEntryKind {
    /// Java: public final int tag
    pub const fn tag(self) -> i32 {
        match self {
            TypePathEntryKind::ARRAY => 0,
            TypePathEntryKind::INNER_TYPE => 1,
            TypePathEntryKind::WILDCARD => 2,
            TypePathEntryKind::TYPE_ARGUMENT => 3,
        }
    }
}

/// Java: public static class TypePathEntry
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypePathEntry {
    /// Java: public final TypePathEntryKind tag
    pub tag: TypePathEntryKind,
    /// Java: public final int arg
    pub arg: i32,
}

impl TypePathEntry {
    /// Java: public static final int bytesPerEntry = 2
    pub const BYTES_PER_ENTRY: i32 = 2;
    /// Java: public static final TypePathEntry ARRAY
    pub const ARRAY: TypePathEntry = TypePathEntry { tag: TypePathEntryKind::ARRAY, arg: 0 };
    /// Java: public static final TypePathEntry INNER_TYPE
    pub const INNER_TYPE: TypePathEntry = TypePathEntry { tag: TypePathEntryKind::INNER_TYPE, arg: 0 };
    /// Java: public static final TypePathEntry WILDCARD
    pub const WILDCARD: TypePathEntry = TypePathEntry { tag: TypePathEntryKind::WILDCARD, arg: 0 };

    /// Java: private TypePathEntry(TypePathEntryKind tag) / the public
    /// TypePathEntry(TypePathEntryKind tag, int arg) — expressed by the
    /// struct fields with the same assertion at from_binary construction.
    ///
    /// Java: public static TypePathEntry fromBinary(int tag, int arg)
    pub fn from_binary(tag: i32, arg: i32) -> TypePathEntry {
        assert!(arg == 0 || tag == TypePathEntryKind::TYPE_ARGUMENT.tag());
        match tag {
            0 => TypePathEntry::ARRAY,
            1 => TypePathEntry::INNER_TYPE,
            2 => TypePathEntry::WILDCARD,
            3 => TypePathEntry {
                tag: TypePathEntryKind::TYPE_ARGUMENT,
                arg,
            },
            _ => panic!("Invalid TypePathEntryKind tag: {tag}"),
        }
    }
}

impl std::fmt::Display for TypePathEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tag = match self.tag {
            TypePathEntryKind::ARRAY => "ARRAY",
            TypePathEntryKind::INNER_TYPE => "INNER_TYPE",
            TypePathEntryKind::WILDCARD => "WILDCARD",
            TypePathEntryKind::TYPE_ARGUMENT => "TYPE_ARGUMENT",
        };
        if self.tag == TypePathEntryKind::TYPE_ARGUMENT {
            write!(f, "{tag}({})", self.arg)
        } else {
            write!(f, "{tag}")
        }
    }
}

/// Java: public class TypeAnnotationPosition
#[derive(Clone, Debug)]
pub struct TypeAnnotationPosition {
    /// Java: public final TargetType type
    pub type_: TargetType,
    /// Java: public List<TypePathEntry> location
    pub location: List<TypePathEntry>,
    /// Java: public final int pos
    pub pos: i32,
    /// Java: public boolean isValidOffset = false
    pub is_valid_offset: bool,
    /// Java: public int offset = -1
    pub offset: i32,
    /// Java: public int[] lvarOffset = null
    pub lvar_offset: Option<Vec<i32>>,
    /// Java: public int[] lvarLength = null
    pub lvar_length: Option<Vec<i32>>,
    /// Java: public int[] lvarIndex = null
    pub lvar_index: Option<Vec<i32>>,
    /// Java: public final int bound_index
    pub bound_index: i32,
    /// Java: public int parameter_index
    pub parameter_index: i32,
    /// Java: public final int type_index
    pub type_index: i32,
    /// Java: private int exception_index = Integer.MIN_VALUE
    pub exception_index: i32,
    /// Java: private int exceptionStartPos = Integer.MIN_VALUE
    pub exception_start_pos: i32,
    /// Java: public final JCLambda onLambda -> tree arena id
    pub on_lambda: Option<TreeId>,
}

impl TypeAnnotationPosition {
    /// Java: public static final List<TypePathEntry> emptyPath = List.nil()
    pub const INT_MIN: i32 = i32::MIN;

    /// Java: private TypeAnnotationPosition(TargetType ttype, int pos,
    /// int parameter_index, JCLambda onLambda, int type_index, int bound_index,
    /// List<TypePathEntry> location)
    fn new(
        type_: TargetType,
        pos: i32,
        parameter_index: i32,
        on_lambda: Option<TreeId>,
        type_index: i32,
        bound_index: i32,
        location: List<TypePathEntry>,
    ) -> TypeAnnotationPosition {
        TypeAnnotationPosition {
            type_,
            location,
            pos,
            is_valid_offset: false,
            offset: -1,
            lvar_offset: None,
            lvar_length: None,
            lvar_index: None,
            bound_index,
            parameter_index,
            type_index,
            exception_index: i32::MIN,
            exception_start_pos: i32::MIN,
            on_lambda,
        }
    }

    pub fn empty_path() -> List<TypePathEntry> {
        List::nil()
    }

    /// Java: public static TypeAnnotationPosition method_return(List<TypePathEntry> location, int pos)
    pub fn method_return(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_RETURN, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_receiver(List<TypePathEntry> location, int pos)
    pub fn method_receiver(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_RECEIVER, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_parameter(List<TypePathEntry> location, int parameter_index, int pos)
    pub fn method_parameter(location: List<TypePathEntry>, parameter_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_FORMAL_PARAMETER, pos, parameter_index, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_ref(List<TypePathEntry> location, int pos)
    pub fn method_ref(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_REFERENCE, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition constructor_ref(List<TypePathEntry> location, int pos)
    pub fn constructor_ref(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CONSTRUCTOR_REFERENCE, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition field(List<TypePathEntry> location, int pos)
    pub fn field(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::FIELD, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition local_variable(List<TypePathEntry> location, int pos)
    pub fn local_variable(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::LOCAL_VARIABLE, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition resource_variable(List<TypePathEntry> location, int pos)
    pub fn resource_variable(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::RESOURCE_VARIABLE, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition exception_parameter(List<TypePathEntry> location, int pos)
    pub fn exception_parameter(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::EXCEPTION_PARAMETER, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition new_obj(List<TypePathEntry> location, int pos)
    pub fn new_obj(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::NEW, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition instance_of(List<TypePathEntry> location, int pos)
    pub fn instance_of(location: List<TypePathEntry>, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::INSTANCEOF, pos, i32::MIN, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition class_extends(List<TypePathEntry> location, int type_index, int pos)
    pub fn class_extends(location: List<TypePathEntry>, type_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CLASS_EXTENDS, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_throws(List<TypePathEntry> location, int type_index, int pos)
    pub fn method_throws(location: List<TypePathEntry>, type_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::THROWS, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition typeCast(List<TypePathEntry> location, int type_index, int pos)
    pub fn type_cast(
        location: List<TypePathEntry>,
        type_index: i32,
        pos: i32,
        on_lambda: Option<crate::ids::TreeId>,
    ) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CAST, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_invocation_type_arg(List<TypePathEntry> location, int type_index, int pos)
    pub fn method_invocation_type_arg(location: List<TypePathEntry>, type_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_INVOCATION_TYPE_ARGUMENT, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition constructor_invocation_type_arg(List<TypePathEntry> location, int type_index, int pos)
    pub fn constructor_invocation_type_arg(location: List<TypePathEntry>, type_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_ref_type_arg(List<TypePathEntry> location, int type_index, int pos)
    pub fn method_ref_type_arg(location: List<TypePathEntry>, type_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_REFERENCE_TYPE_ARGUMENT, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition constructor_ref_type_arg(List<TypePathEntry> location, int type_index, int pos)
    pub fn constructor_ref_type_arg(location: List<TypePathEntry>, type_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT, pos, i32::MIN, on_lambda, type_index, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition type_parameter(List<TypePathEntry> location, int parameter_index)
    pub fn type_parameter(location: List<TypePathEntry>, parameter_index: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CLASS_TYPE_PARAMETER, -1, parameter_index, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition method_type_parameter(List<TypePathEntry> location, int parameter_index)
    pub fn method_type_parameter(location: List<TypePathEntry>, parameter_index: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_TYPE_PARAMETER, -1, parameter_index, on_lambda, i32::MIN, i32::MIN, location)
    }

    /// Java: public static TypeAnnotationPosition type_parameter_bound(List<TypePathEntry> location, int parameter_index, int bound_index)
    pub fn type_parameter_bound(location: List<TypePathEntry>, parameter_index: i32, bound_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::CLASS_TYPE_PARAMETER_BOUND, pos, parameter_index, on_lambda, i32::MIN, bound_index, location)
    }

    /// Java: public static TypeAnnotationPosition method_type_parameter_bound(List<TypePathEntry> location, int parameter_index, int bound_index)
    pub fn method_type_parameter_bound(location: List<TypePathEntry>, parameter_index: i32, bound_index: i32, pos: i32, on_lambda: Option<crate::ids::TreeId>) -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::METHOD_TYPE_PARAMETER_BOUND, pos, parameter_index, on_lambda, i32::MIN, bound_index, location)
    }

    /// Java: public boolean emitToClassfile()
    pub fn emit_to_classfile(&self) -> bool {
        !self.type_.is_local() || self.is_valid_offset
    }

    /// Java: public boolean matchesPos(int pos)
    pub fn matches_pos(&self, pos: i32) -> bool {
        self.pos == pos
    }

    /// Java: public void updatePosOffset(int to)
    pub fn update_pos_offset(&mut self, to: i32) {
        self.offset = to;
        self.is_valid_offset = true;
    }

    /// Java: public boolean hasExceptionIndex()
    pub fn has_exception_index(&self) -> bool {
        self.exception_index >= 0
    }

    /// Java: public int getExceptionIndex()
    pub fn get_exception_index(&self) -> i32 {
        assert!(self.exception_index >= 0, "exception_index is not set");
        self.exception_index
    }

    /// Java: public void setExceptionIndex(final int exception_index)
    pub fn set_exception_index(&mut self, exception_index: i32) {
        assert!(!self.has_exception_index(), "exception_index already set");
        assert!(exception_index >= 0, "Expected a valid index into exception table");
        self.exception_index = exception_index;
        self.is_valid_offset = true;
    }

    /// Java: public boolean hasCatchType()
    pub fn has_catch_type(&self) -> bool {
        self.exception_index < 0 && self.exception_index != i32::MIN
    }

    /// Java: public int getCatchType()
    pub fn get_catch_type(&self) -> i32 {
        assert!(self.has_catch_type(), "exception_index does not contain valid catch info");
        (-self.exception_index) - 1
    }

    /// Java: public int getStartPos()
    pub fn get_start_pos(&self) -> i32 {
        assert!(self.exception_start_pos >= 0, "exceptionStartPos does not contain valid start position");
        self.exception_start_pos
    }

    /// Java: public void setCatchInfo(final int catchType, final int startPos)
    pub fn set_catch_info(&mut self, catch_type: i32, start_pos: i32) {
        assert!(!self.has_exception_index(), "exception_index is already set");
        assert!(catch_type >= 0, "Expected a valid catch type");
        assert!(start_pos >= 0, "Expected a valid start position");
        self.exception_index = -(catch_type + 1);
        self.exception_start_pos = start_pos;
    }

    /// Java: public static List<TypePathEntry> getTypePathFromBinary(java.util.List<Integer> list)
    pub fn get_type_path_from_binary(mut list: List<i32>) -> List<TypePathEntry> {
        let mut loc = List::nil();
        while !list.is_empty() {
            assert!(list.tail().non_empty());
            let tag = *list.head();
            list = list.tail();
            let arg = *list.head();
            list = list.tail();
            loc = loc.append(TypePathEntry::from_binary(tag, arg));
        }
        loc
    }

    /// Java: public static List<Integer> getBinaryFromTypePath(java.util.List<TypePathEntry> locs)
    pub fn get_binary_from_type_path(mut locs: List<TypePathEntry>) -> List<i32> {
        let mut loc = List::nil();
        while !locs.is_empty() {
            let tpe = *locs.head();
            loc = loc.append(tpe.tag.tag());
            loc = loc.append(tpe.arg);
            locs = locs.tail();
        }
        loc
    }

    /// Java: public static final TypeAnnotationPosition unknown = new TypeAnnotationPosition(TargetType.UNKNOWN)
    pub fn unknown() -> TypeAnnotationPosition {
        TypeAnnotationPosition::new(TargetType::UNKNOWN, -1, i32::MIN, None, i32::MIN, i32::MIN, List::nil())
    }
}
