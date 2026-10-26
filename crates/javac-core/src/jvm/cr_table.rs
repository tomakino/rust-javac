// Source: com/sun/tools/javac/jvm/CRTable.java

//! CharacterRangeTable for a method: maps trees to source ranges and writes the
//! CharacterRangeTable attribute entries.

use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::jc_tree::{TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::util::byte_buffer::ByteBuffer;
use crate::util::jc_diagnostic::Warning;
use crate::util::log::Log;
use crate::util::position::{self, LineMap, NOPOS};

/// One character-range-table entry.
#[derive(Clone, Debug)]
pub struct CrtEntry {
    /// The tree or tree list whose source range is recorded.
    pub tree: CrtKey,
    /// The CRT flags of the entry.
    pub flags: i32,
    /// The first code position covered by the entry.
    pub start_pc: i32,
    /// The end code position (exclusive) covered by the entry.
    pub end_pc: i32,
}

/// The key used to map trees and tree lists to their source ranges.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CrtKey {
    /// A single tree.
    Tree(TreeId),
    /// A tree list.
    List(Vec<TreeId>),
}

/// The source range of a tree or tree list.
#[derive(Clone, Debug)]
pub struct SourceRange {
    /// The first source position.
    pub start_pos: i32,
    /// The last source position.
    pub end_pos: i32,
}

impl Default for SourceRange {
    /// Create an empty source range (Java: the no-argument constructor).
    fn default() -> SourceRange {
        SourceRange::new()
    }
}

impl SourceRange {
    /// Create an empty source range.
    pub fn new() -> SourceRange {
        SourceRange {
            start_pos: NOPOS,
            end_pos: NOPOS,
        }
    }

    /// Create a source range from two positions.
    pub fn with_positions(start_pos: i32, end_pos: i32) -> SourceRange {
        SourceRange { start_pos, end_pos }
    }

    /// Merge another range into this one, keeping the widest extent.
    pub fn merge_with(&mut self, other: Option<&SourceRange>) {
        let Some(other) = other else {
            return;
        };
        if self.start_pos == NOPOS {
            self.start_pos = other.start_pos;
        } else if other.start_pos != NOPOS {
            self.start_pos = if self.start_pos < other.start_pos {
                self.start_pos
            } else {
                other.start_pos
            };
        }
        if self.end_pos == NOPOS {
            self.end_pos = other.end_pos;
        } else if other.end_pos != NOPOS {
            self.end_pos = if self.end_pos > other.end_pos {
                self.end_pos
            } else {
                other.end_pos
            };
        }
    }
}

/// The CharacterRangeTable of one method.
pub struct CRTable {
    /// Whether debug output is produced.
    crt_debug: bool,
    /// The collected entries.
    pub entries: Vec<CrtEntry>,
    /// The source ranges of trees and tree lists.
    pub positions: HashMap<CrtKey, SourceRange>,
    /// The end-position table of the compilation unit.
    pub end_pos_table: Option<Rc<dyn EndPosTable>>,
    /// The method declaration this table describes.
    pub method_tree: TreeId,
}

impl CRTable {
    /// Create a table for a method declaration and its end-position table.
    pub fn new(method_tree: TreeId, end_pos_table: Option<Rc<dyn EndPosTable>>) -> CRTable {
        CRTable {
            crt_debug: false,
            entries: Vec::new(),
            positions: HashMap::new(),
            end_pos_table,
            method_tree,
        }
    }

    /// Add a character-range-table entry.
    pub fn put(&mut self, tree: CrtKey, flags: i32, start_pc: i32, end_pc: i32) {
        self.entries.push(CrtEntry {
            tree,
            flags,
            start_pc,
            end_pc,
        });
    }

    /// Return the number of entries.
    pub fn length(&self) -> i32 {
        self.entries.len() as i32
    }

    /// Compute the source positions and write the table into the buffer.
    ///
    /// The method returns the number of written entries; entries without bytecode
    /// (empty blocks/statements) or without defined positions are skipped, and a
    /// position that cannot be encoded is reported as a warning and skipped.
    pub fn write_crt(
        &mut self,
        ctx: &mut CompilerContext,
        databuf: &mut ByteBuffer,
        line_map: &dyn LineMap,
        log: &Rc<std::cell::RefCell<Log>>,
    ) -> i32 {
        let mut crt_entries = 0;
        let mut computer = SourceComputer {
            result: None,
            positions: std::mem::take(&mut self.positions),
            end_pos_table: self.end_pos_table.clone(),
        };
        computer.csp(ctx, Some(self.method_tree));
        self.positions = std::mem::take(&mut computer.positions);
        let entries = self.entries.clone();
        for entry in entries {
            if entry.start_pc == entry.end_pc {
                continue;
            }
            let pos = match self.positions.get(&entry.tree) {
                Some(pos) => pos.clone(),
                None => panic!("AssertionError: CRT: tree source positions are undefined"),
            };
            if pos.start_pos == NOPOS || pos.end_pos == NOPOS {
                continue;
            }
            if self.crt_debug {
                println!(
                    "Tree: {:?}, type:{}",
                    entry.tree,
                    get_types(entry.flags)
                );
                println!("Start: pos = {}, pc = {}", pos.start_pos, entry.start_pc);
            }
            let start_pos = self.encode_position(pos.start_pos, line_map, log);
            if start_pos == NOPOS {
                continue;
            }
            if self.crt_debug {
                println!("End:   pos = {}, pc = {}", pos.end_pos, entry.end_pc - 1);
            }
            let end_pos = self.encode_position(pos.end_pos, line_map, log);
            if end_pos == NOPOS {
                continue;
            }
            databuf.append_char(entry.start_pc);
            databuf.append_char(entry.end_pc - 1);
            databuf.append_int(start_pos);
            databuf.append_int(end_pos);
            databuf.append_char(entry.flags);
            crt_entries += 1;
        }
        crt_entries
    }

    /// Encode a source position as line/column.
    fn encode_position(
        &self,
        pos: i32,
        line_map: &dyn LineMap,
        log: &Rc<std::cell::RefCell<Log>>,
    ) -> i32 {
        let line = line_map.get_line_number(pos);
        let col = line_map.get_column_number(pos);
        let new_pos = position::encode_position(line, col);
        if self.crt_debug {
            println!(", line = {}, column = {}, new_pos = {}", line, col, new_pos);
        }
        if new_pos == NOPOS {
            let warning = Warning::from(
                crate::resources::compiler_properties::warnings::position_overflow(line),
            );
            crate::util::abstract_log::AbstractLog::warning_at(
                &mut *log.borrow_mut(),
                pos,
                warning,
            );
        }
        new_pos
    }
}

/// Return a textual description of the CRT flags.
fn get_types(flags: i32) -> String {
    let mut types = String::new();
    if (flags & crate::jvm::crt_flags::CRT_STATEMENT) != 0 {
        types.push_str(" CRT_STATEMENT");
    }
    if (flags & crate::jvm::crt_flags::CRT_BLOCK) != 0 {
        types.push_str(" CRT_BLOCK");
    }
    if (flags & crate::jvm::crt_flags::CRT_ASSIGNMENT) != 0 {
        types.push_str(" CRT_ASSIGNMENT");
    }
    if (flags & crate::jvm::crt_flags::CRT_FLOW_CONTROLLER) != 0 {
        types.push_str(" CRT_FLOW_CONTROLLER");
    }
    if (flags & crate::jvm::crt_flags::CRT_FLOW_TARGET) != 0 {
        types.push_str(" CRT_FLOW_TARGET");
    }
    if (flags & crate::jvm::crt_flags::CRT_INVOKE) != 0 {
        types.push_str(" CRT_INVOKE");
    }
    if (flags & crate::jvm::crt_flags::CRT_CREATE) != 0 {
        types.push_str(" CRT_CREATE");
    }
    if (flags & crate::jvm::crt_flags::CRT_BRANCH_TRUE) != 0 {
        types.push_str(" CRT_BRANCH_TRUE");
    }
    if (flags & crate::jvm::crt_flags::CRT_BRANCH_FALSE) != 0 {
        types.push_str(" CRT_BRANCH_FALSE");
    }
    types
}

/// Computes source ranges for the trees of one method.
struct SourceComputer {
    /// The range of the most recently visited tree.
    pub result: Option<SourceRange>,
    /// The collected tree positions.
    pub positions: HashMap<CrtKey, SourceRange>,
    /// The end-position table of the compilation unit.
    pub end_pos_table: Option<Rc<dyn EndPosTable>>,
}

impl SourceComputer {
    /// Compute and record the source range of a single tree.
    fn csp(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<SourceRange> {
        let tree = tree?;
        crate::tree::jc_tree::accept(ctx, tree, self);
        let result = self.result.clone();
        if let Some(range) = &result {
            self.positions.insert(CrtKey::Tree(tree), range.clone());
        }
        result
    }

    /// Compute and record the source range of a tree list.
    fn csp_list(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &crate::util::list::List<TreeId>,
    ) -> Option<SourceRange> {
        if trees.is_empty() {
            return None;
        }
        let mut list_sr = SourceRange::new();
        let mut current = trees.clone();
        while current.non_empty() {
            let head = *current.head();
            list_sr.merge_with(self.csp(ctx, Some(head)).as_ref());
            current = current.tail();
        }
        self.positions
            .insert(CrtKey::List(trees.iter().collect()), list_sr.clone());
        Some(list_sr)
    }

    /// Compute and record the source range of a case list.
    fn csp_cases(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &crate::util::list::List<TreeId>,
    ) -> Option<SourceRange> {
        self.csp_list(ctx, trees)
    }

    /// Compute and record the source range of a catch-clause list.
    fn csp_catchers(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &crate::util::list::List<TreeId>,
    ) -> Option<SourceRange> {
        self.csp_list(ctx, trees)
    }

    /// Return the start position of a tree.
    fn start_pos(&self, ctx: &CompilerContext, tree: Option<TreeId>) -> i32 {
        let Some(tree) = tree else {
            return NOPOS;
        };
        tree_info::get_start_pos(ctx, tree)
    }

    /// Return the end position of a tree, or NOPOS when undefined.
    fn end_pos(&self, ctx: &CompilerContext, tree: Option<TreeId>) -> i32 {
        let Some(tree) = tree else {
            return NOPOS;
        };
        let end_pos_table = self.end_pos_table.as_deref();
        tree_info::get_end_pos(ctx, Some(tree), end_pos_table)
    }

    /// Create the source range covering the whole tree.
    fn range(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
    ) -> SourceRange {
        SourceRange::with_positions(self.start_pos(ctx, Some(tree)), self.end_pos(ctx, Some(tree)))
    }
}

impl TreeVisitor for SourceComputer {
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let body = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { body, .. } => *body,
            _ => panic!("expected a method declaration"),
        };
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { vartype, init, .. } => (*vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        self.csp(ctx, vartype);
        sr.merge_with(self.csp(ctx, init).as_ref());
        self.result = Some(sr);
    }

    fn visit_skip(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let start = self.start_pos(ctx, Some(tree));
        self.result = Some(SourceRange::with_positions(start, start));
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sr = self.range(ctx, tree);
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        self.csp_list(ctx, &stats);
        self.result = Some(sr);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do-while loop"),
        };
        sr.merge_with(self.csp(ctx, body).as_ref());
        sr.merge_with(self.csp(ctx, cond).as_ref());
        self.result = Some(sr);
    }
    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop {
                cond,
                body,
                ..
            } => (
                *cond,
                *body
            ),
            _ => panic!("expected a while loop"),
        };
        sr.merge_with(self.csp(ctx, cond).as_ref());
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
                ..
            } => (
                init.clone(),
                *cond,
                step.clone(),
                *body
            ),
            _ => panic!("expected a for loop"),
        };
        sr.merge_with(self.csp_list(ctx, &init).as_ref());
        sr.merge_with(self.csp(ctx, cond).as_ref());
        sr.merge_with(self.csp_list(ctx, &step).as_ref());
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop {
                var,
                expr,
                body,
                ..
            } => (
                *var,
                *expr,
                *body
            ),
            _ => panic!("expected a enhanced for loop"),
        };
        sr.merge_with(self.csp(ctx, var).as_ref());
        sr.merge_with(self.csp(ctx, expr).as_ref());
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement {
                body,
                ..
            } => *body,
            _ => panic!("expected a labeled statement"),
        };
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector,
                cases,
                ..
            } => (
                *selector,
                cases.clone()
            ),
            _ => panic!("expected a switch statement"),
        };
        sr.merge_with(self.csp(ctx, selector).as_ref());
        sr.merge_with(self.csp_cases(ctx, &cases).as_ref());
        self.result = Some(sr);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector,
                cases,
                ..
            } => (
                *selector,
                cases.clone()
            ),
            _ => panic!("expected a switch expression"),
        };
        sr.merge_with(self.csp(ctx, selector).as_ref());
        sr.merge_with(self.csp_cases(ctx, &cases).as_ref());
        self.result = Some(sr);
    }

    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case {
                labels,
                stats,
                ..
            } => (
                labels.clone(),
                stats.clone()
            ),
            _ => panic!("expected a case"),
        };
        sr.merge_with(self.csp_list(ctx, &labels).as_ref());
        sr.merge_with(self.csp_list(ctx, &stats).as_ref());
        self.result = Some(sr);
    }

    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized {
                lock,
                body,
                ..
            } => (
                *lock,
                *body
            ),
            _ => panic!("expected a synchronized statement"),
        };
        sr.merge_with(self.csp(ctx, lock).as_ref());
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (
                resources.clone(),
                *body,
                catchers.clone(),
                *finalizer
            ),
            _ => panic!("expected a try statement"),
        };
        sr.merge_with(self.csp_list(ctx, &resources).as_ref());
        sr.merge_with(self.csp(ctx, body).as_ref());
        sr.merge_with(self.csp_catchers(ctx, &catchers).as_ref());
        sr.merge_with(self.csp(ctx, finalizer).as_ref());
        self.result = Some(sr);
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch {
                param,
                body,
                ..
            } => (
                *param,
                *body
            ),
            _ => panic!("expected a catch clause"),
        };
        sr.merge_with(self.csp(ctx, param).as_ref());
        sr.merge_with(self.csp(ctx, body).as_ref());
        self.result = Some(sr);
    }

    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (
                *cond,
                *truepart,
                *falsepart
            ),
            _ => panic!("expected a conditional expression"),
        };
        sr.merge_with(self.csp(ctx, cond).as_ref());
        sr.merge_with(self.csp(ctx, truepart).as_ref());
        sr.merge_with(self.csp(ctx, falsepart).as_ref());
        self.result = Some(sr);
    }

    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
                ..
            } => (
                *cond,
                *thenpart,
                *elsepart
            ),
            _ => panic!("expected a if statement"),
        };
        sr.merge_with(self.csp(ctx, cond).as_ref());
        sr.merge_with(self.csp(ctx, thenpart).as_ref());
        sr.merge_with(self.csp(ctx, elsepart).as_ref());
        self.result = Some(sr);
    }

    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement {
                expr,
                ..
            } => *expr,
            _ => panic!("expected a expression statement"),
        };
        sr.merge_with(self.csp(ctx, expr).as_ref());
        self.result = Some(sr);
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sr = self.range(ctx, tree);
        self.result = Some(sr);
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield {
                value,
                ..
            } => *value,
            _ => panic!("expected a yield statement"),
        };
        sr.merge_with(self.csp(ctx, value).as_ref());
        self.result = Some(sr);
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sr = self.range(ctx, tree);
        self.result = Some(sr);
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return {
                expr,
                ..
            } => *expr,
            _ => panic!("expected a return statement"),
        };
        sr.merge_with(self.csp(ctx, expr).as_ref());
        self.result = Some(sr);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw {
                expr,
                ..
            } => *expr,
            _ => panic!("expected a throw statement"),
        };
        sr.merge_with(self.csp(ctx, expr).as_ref());
        self.result = Some(sr);
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert {
                cond,
                detail,
                ..
            } => (
                *cond,
                *detail
            ),
            _ => panic!("expected a assert statement"),
        };
        sr.merge_with(self.csp(ctx, cond).as_ref());
        sr.merge_with(self.csp(ctx, detail).as_ref());
        self.result = Some(sr);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                meth,
                args,
                ..
            } => (
                *meth,
                args.clone()
            ),
            _ => panic!("expected a method invocation"),
        };
        sr.merge_with(self.csp(ctx, meth).as_ref());
        sr.merge_with(self.csp_list(ctx, &args).as_ref());
        self.result = Some(sr);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (encl, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                ..
            } => (
                *encl,
                *clazz,
                args.clone(),
                *def
            ),
            _ => panic!("expected a new class expression"),
        };
        sr.merge_with(self.csp(ctx, encl).as_ref());
        sr.merge_with(self.csp(ctx, clazz).as_ref());
        sr.merge_with(self.csp_list(ctx, &args).as_ref());
        sr.merge_with(self.csp(ctx, def).as_ref());
        self.result = Some(sr);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (elemtype, dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray {
                elemtype,
                dims,
                elems,
                ..
            } => (
                *elemtype,
                dims.clone(),
                elems.clone()
            ),
            _ => panic!("expected a new array expression"),
        };
        sr.merge_with(self.csp(ctx, elemtype).as_ref());
        sr.merge_with(self.csp_list(ctx, &dims).as_ref());
        if let Some(list) = elems { sr.merge_with(self.csp_list(ctx, &list).as_ref()); }
        self.result = Some(sr);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens {
                expr,
                ..
            } => *expr,
            _ => panic!("expected a parenthesized expression"),
        };
        sr.merge_with(self.csp(ctx, expr).as_ref());
        self.result = Some(sr);
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign {
                lhs,
                rhs,
                ..
            } => (
                *lhs,
                *rhs
            ),
            _ => panic!("expected a assignment"),
        };
        sr.merge_with(self.csp(ctx, lhs).as_ref());
        sr.merge_with(self.csp(ctx, rhs).as_ref());
        self.result = Some(sr);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp {
                lhs,
                rhs,
                ..
            } => (
                *lhs,
                *rhs
            ),
            _ => panic!("expected a compound assignment"),
        };
        sr.merge_with(self.csp(ctx, lhs).as_ref());
        sr.merge_with(self.csp(ctx, rhs).as_ref());
        self.result = Some(sr);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary {
                arg,
                ..
            } => *arg,
            _ => panic!("expected a unary expression"),
        };
        sr.merge_with(self.csp(ctx, arg).as_ref());
        self.result = Some(sr);
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary {
                lhs,
                rhs,
                ..
            } => (
                *lhs,
                *rhs
            ),
            _ => panic!("expected a binary expression"),
        };
        sr.merge_with(self.csp(ctx, lhs).as_ref());
        sr.merge_with(self.csp(ctx, rhs).as_ref());
        self.result = Some(sr);
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast {
                clazz,
                expr,
                ..
            } => (
                *clazz,
                *expr
            ),
            _ => panic!("expected a type cast"),
        };
        sr.merge_with(self.csp(ctx, clazz).as_ref());
        sr.merge_with(self.csp(ctx, expr).as_ref());
        self.result = Some(sr);
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf {
                expr,
                pattern,
                ..
            } => (
                *expr,
                *pattern
            ),
            _ => panic!("expected a instanceof expression"),
        };
        sr.merge_with(self.csp(ctx, expr).as_ref());
        sr.merge_with(self.csp(ctx, pattern).as_ref());
        self.result = Some(sr);
    }

    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess {
                indexed,
                index,
                ..
            } => (
                *indexed,
                *index
            ),
            _ => panic!("expected a array access"),
        };
        sr.merge_with(self.csp(ctx, indexed).as_ref());
        sr.merge_with(self.csp(ctx, index).as_ref());
        self.result = Some(sr);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess {
                selected,
                ..
            } => *selected,
            _ => panic!("expected a field access"),
        };
        sr.merge_with(self.csp(ctx, selected).as_ref());
        self.result = Some(sr);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sr = self.range(ctx, tree);
        self.result = Some(sr);
    }

    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sr = self.range(ctx, tree);
        self.result = Some(sr);
    }

    fn visit_type_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sr = self.range(ctx, tree);
        self.result = Some(sr);
    }

    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree {
                elemtype,
                ..
            } => *elemtype,
            _ => panic!("expected a array type"),
        };
        sr.merge_with(self.csp(ctx, elemtype).as_ref());
        self.result = Some(sr);
    }

    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply {
                clazz,
                arguments,
                ..
            } => (
                *clazz,
                arguments.clone()
            ),
            _ => panic!("expected a parameterized type"),
        };
        sr.merge_with(self.csp(ctx, clazz).as_ref());
        sr.merge_with(self.csp_list(ctx, &arguments).as_ref());
        self.result = Some(sr);
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr {
                defs,
                expr,
                ..
            } => (
                defs.clone(),
                *expr
            ),
            _ => panic!("expected a let expression"),
        };
        sr.merge_with(self.csp_list(ctx, &defs).as_ref());
        sr.merge_with(self.csp(ctx, expr).as_ref());
        self.result = Some(sr);
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                bounds,
                ..
            } => bounds.clone(),
            _ => panic!("expected a type parameter"),
        };
        sr.merge_with(self.csp_list(ctx, &bounds).as_ref());
        self.result = Some(sr);
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut sr = self.range(ctx, tree);
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion {
                alternatives,
                ..
            } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        sr.merge_with(self.csp_list(ctx, &alternatives).as_ref());
        self.result = Some(sr);
    }

    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        self.result = None;
    }

    fn visit_wildcard(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        self.result = None;
    }

    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        self.result = None;
    }

    fn visit_tree(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        panic!("AssertionError: unexpected tree in the character range table computer")
    }
}

