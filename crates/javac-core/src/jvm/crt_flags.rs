// Source: com/sun/tools/javac/jvm/CRTFlags.java

//! Character range table flags, indicating the kind of source entry a
//! character-range-table entry describes.

/// Marks a statement entry.
pub const CRT_STATEMENT: i32 = 0x0001;
/// Marks a block entry.
pub const CRT_BLOCK: i32 = 0x0002;
/// Marks an assignment entry.
pub const CRT_ASSIGNMENT: i32 = 0x0004;
/// Marks a flow controller entry.
pub const CRT_FLOW_CONTROLLER: i32 = 0x0008;
/// Marks a flow target entry.
pub const CRT_FLOW_TARGET: i32 = 0x0010;
/// Marks an invocation entry.
pub const CRT_INVOKE: i32 = 0x0020;
/// Marks a creation entry.
pub const CRT_CREATE: i32 = 0x0040;
/// Marks a true branch entry.
pub const CRT_BRANCH_TRUE: i32 = 0x0080;
/// Marks a false branch entry.
pub const CRT_BRANCH_FALSE: i32 = 0x0100;

/// The mask of all valid character-range-table flags.
pub const CRT_VALID_FLAGS: i32 = CRT_STATEMENT
    | CRT_BLOCK
    | CRT_ASSIGNMENT
    | CRT_FLOW_CONTROLLER
    | CRT_FLOW_TARGET
    | CRT_INVOKE
    | CRT_CREATE
    | CRT_BRANCH_TRUE
    | CRT_BRANCH_FALSE;
