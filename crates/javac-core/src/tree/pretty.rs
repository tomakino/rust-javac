//! Prints out a tree as an indented Java source program.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! ADR 0001 (arena + IDs):
//! - the tree graph is printed through the CompilerContext; the upstream
//!   JCTree.Visitor dispatch is reproduced by Pretty::dispatch (tag -> visit
//!   method); the visitor trait itself lands in the visitor;
//! - Java IOException/UncheckedIOException become infallible: the output is an
//!   in-memory JavaString buffer (Java Writer + StringWriter semantics), so the
//!   copied algorithms drop their try/catch (registered section 9);
//! - print(Object) escapes non-ASCII characters with Convert.escapeUnicode,
//!   exactly like the upstream method;
//! - lineSep is "\n" (platform independent output; the oracle comparison is
//!   normalized; registered section 9);
//! - the docComments table is shared as Rc<dyn DocCommentTable> (the
//!   JCCompilationUnit field was changed from Box to Rc so printUnit can keep
//!   the table while printing);
//! - REGISTERED dependency: printUnit(tree, cdef) import filtering needs the
//!   upstream UsedVisitor (TreeScanner subclass overriding visitIdent). The
//!   TreeScanner is ported, but Pretty has no visitor subclass in this file;
//!   the cdef != null path is an explicit registered unimplemented path, while
//!   the cdef == null path (tree pretty oracle) is fully ported.
//!
//! The full Pretty output is compared against the tools/gen-tree-oracle data
//! (JavacParser/TreeMaker are ported; the data covers the modern syntax samples).

// The upstream visitor algorithms are copied with their exact if/match nesting;
// the clippy style suggestions to collapse them are not applied (registered in
// control flow is kept 1:1 with the upstream source).
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]

use std::rc::Rc;

use crate::code::bound_kind::BoundKind;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::doc_comment_table::DocCommentTable;
use crate::tree::jc_tree::{LiteralValue, Tag, TreeKind};
use crate::tree::tree_info;
use crate::util::convert;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::name::Name;

/// The string sequence used when the output is truncated by toSimpleString.
const TRIM_SEQUENCE: &str = "[...]";

/// Max number of chars to be generated when output should fit into one line.
const PREFERRED_LENGTH: i32 = 20;

/// Prints out a tree as an indented Java source program.
pub struct Pretty {
    /// The output buffer (Java: Writer out).
    out: JavaString,
    /// Set when we are producing source output.
    source_output: bool,
    /// Indentation width (can be reassigned from outside).
    pub width: i32,
    /// The current left margin.
    lmargin: i32,
    /// The enclosing class name.
    encl_class_name: Option<Name>,
    /// A table mapping trees to their documentation comments (can be None).
    doc_comments: Option<Rc<std::cell::RefCell<dyn DocCommentTable>>>,
    /// Visitor argument: the current precedence level.
    prec: i32,
}

impl Pretty {
    /// Java: public Pretty(Writer out, boolean sourceOutput)
    pub fn new(source_output: bool) -> Pretty {
        Pretty {
            out: JavaString::from(""),
            source_output,
            width: 4,
            lmargin: 0,
            encl_class_name: None,
            doc_comments: None,
            prec: 0,
        }
    }

    /// The accumulated output (Java: the Writer contents).
    pub fn get_out(&self) -> JavaString {
        self.out.clone()
    }

    /// Java: public static String toSimpleString(JCTree tree)
    pub fn to_simple_string(ctx: &CompilerContext, tree: TreeId) -> JavaString {
        Pretty::to_simple_string_max(ctx, tree, PREFERRED_LENGTH)
    }

    /// Java: public static String toSimpleString(JCTree tree, int maxLength)
    pub fn to_simple_string_max(
        ctx: &CompilerContext,
        tree: TreeId,
        max_length: i32,
    ) -> JavaString {
        let mut pretty = Pretty::new(false);
        pretty.print_expr(ctx, tree);
        // we need to (i) replace all line terminators with a space and (ii)
        // remove occurrences of 'missing' in the Pretty output (generated when
        // types are missing)
        let res = pretty.out.to_string_lossy().trim().to_string();
        let mut normalized = String::new();
        let mut last_space = false;
        for ch in res.chars() {
            if ch.is_whitespace() {
                if !last_space {
                    normalized.push(' ');
                }
                last_space = true;
            } else {
                normalized.push(ch);
                last_space = false;
            }
        }
        let res = normalized.replace("/*missing*/", "");
        let max_length = max_length as usize;
        if res.chars().count() < max_length {
            JavaString::from(res.as_str())
        } else {
            let chars: Vec<char> = res.chars().collect();
            let trim_len = TRIM_SEQUENCE.chars().count();
            let head = (max_length - trim_len) * 2 / 3;
            let tail = max_length - trim_len - head;
            let mut out = String::new();
            out.extend(&chars[..head]);
            out.push_str(TRIM_SEQUENCE);
            out.extend(&chars[chars.len() - tail..]);
            JavaString::from(out.as_str())
        }
    }

    /// Align code to be indented to left margin.
    fn align(&mut self) {
        for _ in 0..self.lmargin {
            self.out.push(' ' as u16);
        }
    }

    /// Increase left margin by indentation width.
    fn indent(&mut self) {
        self.lmargin += self.width;
    }

    /// Decrease left margin by indentation width.
    fn undent(&mut self) {
        self.lmargin -= self.width;
    }

    /// Enter a new precedence level. Emit a '(' if new precedence level is less
    /// than precedence level so far.
    fn open(&mut self, context_prec: i32, own_prec: i32) {
        if own_prec < context_prec {
            self.print("(");
        }
    }

    /// Leave precedence level. Emit a ')' if inner precedence level is less than
    /// precedence level we revert to.
    fn close(&mut self, context_prec: i32, own_prec: i32) {
        if own_prec < context_prec {
            self.print(")");
        }
    }

    /// Print string, replacing all non-ascii character with unicode escapes.
    pub fn print(&mut self, s: &str) {
        let escaped = convert::escape_unicode(&JavaString::from(s));
        self.out
            .as_units_mut()
            .extend_from_slice(escaped.as_units());
    }

    /// Print a JavaString, replacing non-ascii characters with unicode escapes.
    pub fn print_java_string(&mut self, s: &JavaString) {
        let escaped = convert::escape_unicode(s);
        self.out
            .as_units_mut()
            .extend_from_slice(escaped.as_units());
    }

    /// Print a Name (Java: print(Object) on a Name).
    pub fn print_name(&mut self, name: &Name) {
        self.print_java_string(&name.to_java_string());
    }

    /// Print new line.
    pub fn println(&mut self) {
        self.print("\n");
    }

    /// Visitor method: print expression tree.
    pub fn print_expr(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_expr_prec(ctx, tree, tree_info::NO_PREC);
    }

    /// Visitor method: print expression tree at the given precedence level.
    pub fn print_expr_prec(&mut self, ctx: &CompilerContext, tree: TreeId, prec: i32) {
        let prev_prec = self.prec;
        self.prec = prec;
        self.dispatch(ctx, tree);
        self.prec = prev_prec;
    }

    /// Print a nullable expression (Java: printExpr(null) prints /*missing*/).
    pub fn print_expr_opt(&mut self, ctx: &CompilerContext, tree: Option<TreeId>) {
        match tree {
            Some(tree) => self.print_expr(ctx, tree),
            None => self.print("/*missing*/"),
        }
    }

    /// Derived visitor method: print statement tree.
    pub fn print_stat(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_expr_prec(ctx, tree, tree_info::NOT_EXPRESSION);
    }

    /// Derived visitor method: print list of expression trees, separated by the
    /// given string.
    pub fn print_exprs(&mut self, ctx: &CompilerContext, trees: &List<TreeId>, sep: &str) {
        let mut first = true;
        for tree in trees.iter() {
            if !first {
                self.print(sep);
            }
            self.print_expr(ctx, tree);
            first = false;
        }
    }

    /// Derived visitor method: print pattern.
    pub fn print_pattern(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_expr(ctx, tree);
    }

    /// Derived visitor method: print list of statements, each on a separate
    /// line.
    pub fn print_stats(&mut self, ctx: &CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.align();
            self.print_stat(ctx, tree);
            self.println();
        }
    }

    /// Print a set of modifiers.
    pub fn print_flags(&mut self, flags: i64) {
        if (flags & crate::code::flags::SYNTHETIC as i64) != 0 {
            self.print("/*synthetic*/ ");
        }
        let names = tree_info::flag_names(flags);
        self.print_java_string(&names);
        if (flags & crate::code::flags::ExtendedStandardFlags) != 0 {
            self.print(" ");
        }
        if (flags & crate::code::flags::ANNOTATION as i64) != 0 {
            self.print("@");
        }
    }

    /// Print the annotations on separate lines.
    pub fn print_annotations(&mut self, ctx: &CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.print_stat(ctx, tree);
            self.println();
            self.align();
        }
    }

    /// Print the type annotations, each followed by a space.
    pub fn print_type_annotations(&mut self, ctx: &CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.print_expr(ctx, tree);
            self.print(" ");
        }
    }

    /// Print documentation comment, if it exists.
    pub fn print_doc_comment(&mut self, _ctx: &CompilerContext, tree: TreeId) {
        let Some(doc_comments) = self.doc_comments.clone() else {
            return;
        };
        let Some(dc) = doc_comments.borrow().get_comment_text(tree) else {
            return;
        };
        self.print("/**");
        self.println();
        let len = dc.len();
        let mut pos = 0usize;
        let mut endpos = Pretty::line_end_pos(&dc, pos);
        while pos < len {
            self.align();
            self.print(" *");
            if pos < len && dc.char_at(pos) > ' ' as u16 {
                self.print(" ");
            }
            self.print_java_string(&JavaString::from(&dc.as_units()[pos..endpos]));
            self.println();
            pos = endpos + 1;
            endpos = Pretty::line_end_pos(&dc, pos);
        }
        self.align();
        self.print(" */");
        self.println();
        self.align();
    }

    /// Java: static int lineEndPos(String s, int start)
    fn line_end_pos(s: &JavaString, start: usize) -> usize {
        let units = s.as_units();
        let mut pos = start;
        while pos < units.len() {
            if units[pos] == '\n' as u16 {
                return pos;
            }
            pos += 1;
        }
        units.len()
    }

    /// If type parameter list is non-empty, print it enclosed in brackets.
    pub fn print_type_parameters(&mut self, ctx: &CompilerContext, trees: &List<TreeId>) {
        if trees.non_empty() {
            self.print("<");
            self.print_exprs(ctx, trees, ", ");
            self.print(">");
        }
    }

    /// Print a block.
    pub fn print_block(&mut self, ctx: &CompilerContext, stats: &List<TreeId>) {
        self.print("{");
        self.println();
        self.indent();
        self.print_stats(ctx, stats);
        self.undent();
        self.align();
        self.print("}");
    }

    /// Print an enum body.
    pub fn print_enum_body(&mut self, ctx: &CompilerContext, stats: &List<TreeId>) {
        self.print("{");
        self.println();
        self.indent();
        let mut first = true;
        for stat in stats.iter() {
            if Pretty::is_enumerator(ctx, stat) {
                if !first {
                    self.print(",");
                    self.println();
                }
                self.align();
                self.print_stat(ctx, stat);
                first = false;
            }
        }
        self.print(";");
        self.println();
        for stat in stats.iter() {
            if !Pretty::is_enumerator(ctx, stat) {
                self.align();
                self.print_stat(ctx, stat);
                self.println();
            }
        }
        self.undent();
        self.align();
        self.print("}");
    }

    /// Is the given tree an enumerator definition?
    fn is_enumerator(ctx: &CompilerContext, tree: TreeId) -> bool {
        let node = ctx.tree(tree);
        if !node.has_tag(Tag::VARDEF) {
            return false;
        }
        match &node.kind {
            TreeKind::VariableDecl { mods, .. } => mods
                .map(|mods| match &ctx.tree(mods).kind {
                    TreeKind::Modifiers { flags, .. } => {
                        (flags & crate::code::flags::ENUM as i64) != 0
                    }
                    _ => false,
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Print unit consisting of package clause and import statements in
    /// toplevel, followed by class definition. If class definition is None,
    /// print all definitions in toplevel.
    ///
    /// the cdef != None path needs the isUsed scan (internal
    /// TreeScanner subclass used by isUsed; registered as unimplemented.
    pub fn print_unit(&mut self, ctx: &CompilerContext, tree: TreeId, cdef: Option<TreeId>) {
        let TreeKind::CompilationUnit {
            defs, doc_comments, ..
        } = &ctx.tree(tree).kind
        else {
            panic!("expected a compilation unit");
        };
        self.doc_comments = doc_comments.clone();
        let defs = defs.clone();
        self.print_doc_comment(ctx, tree);
        if cdef.is_some() {
            panic!(
                "REGISTERED: printUnit(tree, cdef) import filtering needs the UsedVisitor TreeScanner subclass"
            );
        }
        let mut first_import = true;
        for def in defs.iter() {
            let node = ctx.tree(def);
            if node.has_tag(Tag::IMPORT) {
                let TreeKind::Import { qualid, .. } = &node.kind else {
                    continue;
                };
                if let Some(qualid) = *qualid {
                    let name = tree_info::name(ctx, qualid);
                    let is_asterisk = name
                        .map(|name| {
                            name.to_java_string() == name.table.names().asterisk.to_java_string()
                        })
                        .unwrap_or(false);
                    if is_asterisk || cdef.is_none() {
                        if first_import {
                            first_import = false;
                            self.println();
                        }
                        self.print_stat(ctx, def);
                    }
                }
            } else {
                self.print_stat(ctx, def);
            }
        }
        if let Some(cdef) = cdef {
            self.print_stat(ctx, cdef);
            self.println();
        }
    }

    /// Dispatch to the visitor method for the tree (upstream: tree.accept(this)).
    fn dispatch(&mut self, ctx: &CompilerContext, tree: TreeId) {
        match ctx.tree(tree).tag {
            Tag::TOPLEVEL => self.visit_top_level(ctx, tree),
            Tag::PACKAGEDEF => self.visit_package_def(ctx, tree),
            Tag::IMPORT => self.visit_import(ctx, tree),
            Tag::CLASSDEF => self.visit_class_def(ctx, tree),
            Tag::METHODDEF => self.visit_method_def(ctx, tree),
            Tag::VARDEF => self.visit_var_def(ctx, tree),
            Tag::SKIP => self.visit_skip(ctx, tree),
            Tag::BLOCK => self.visit_block(ctx, tree),
            Tag::DOLOOP => self.visit_do_loop(ctx, tree),
            Tag::WHILELOOP => self.visit_while_loop(ctx, tree),
            Tag::FORLOOP => self.visit_for_loop(ctx, tree),
            Tag::FOREACHLOOP => self.visit_foreach_loop(ctx, tree),
            Tag::LABELLED => self.visit_labelled(ctx, tree),
            Tag::SWITCH => self.visit_switch(ctx, tree),
            Tag::CASE => self.visit_case(ctx, tree),
            Tag::SWITCH_EXPRESSION => self.visit_switch_expression(ctx, tree),
            Tag::SYNCHRONIZED => self.visit_synchronized(ctx, tree),
            Tag::TRY => self.visit_try(ctx, tree),
            Tag::CATCH => self.visit_catch(ctx, tree),
            Tag::CONDEXPR => self.visit_conditional(ctx, tree),
            Tag::IF => self.visit_if(ctx, tree),
            Tag::EXEC => self.visit_exec(ctx, tree),
            Tag::BREAK => self.visit_break(ctx, tree),
            Tag::YIELD => self.visit_yield(ctx, tree),
            Tag::CONTINUE => self.visit_continue(ctx, tree),
            Tag::RETURN => self.visit_return(ctx, tree),
            Tag::THROW => self.visit_throw(ctx, tree),
            Tag::ASSERT => self.visit_assert(ctx, tree),
            Tag::APPLY => self.visit_apply(ctx, tree),
            Tag::NEWCLASS => self.visit_new_class(ctx, tree),
            Tag::NEWARRAY => self.visit_new_array(ctx, tree),
            Tag::LAMBDA => self.visit_lambda(ctx, tree),
            Tag::PARENS => self.visit_parens(ctx, tree),
            Tag::ASSIGN => self.visit_assign(ctx, tree),
            Tag::BITOR_ASG
            | Tag::BITXOR_ASG
            | Tag::BITAND_ASG
            | Tag::SL_ASG
            | Tag::SR_ASG
            | Tag::USR_ASG
            | Tag::PLUS_ASG
            | Tag::MINUS_ASG
            | Tag::MUL_ASG
            | Tag::DIV_ASG
            | Tag::MOD_ASG => self.visit_assignop(ctx, tree),
            Tag::POS
            | Tag::NEG
            | Tag::NOT
            | Tag::COMPL
            | Tag::PREINC
            | Tag::PREDEC
            | Tag::POSTINC
            | Tag::POSTDEC
            | Tag::NULLCHK => self.visit_unary(ctx, tree),
            Tag::OR
            | Tag::AND
            | Tag::BITOR
            | Tag::BITXOR
            | Tag::BITAND
            | Tag::EQ
            | Tag::NE
            | Tag::LT
            | Tag::GT
            | Tag::LE
            | Tag::GE
            | Tag::SL
            | Tag::SR
            | Tag::USR
            | Tag::PLUS
            | Tag::MINUS
            | Tag::MUL
            | Tag::DIV
            | Tag::MOD => self.visit_binary(ctx, tree),
            Tag::TYPECAST => self.visit_type_cast(ctx, tree),
            Tag::TYPETEST => self.visit_type_test(ctx, tree),
            Tag::BINDINGPATTERN => self.visit_binding_pattern(ctx, tree),
            Tag::DEFAULTCASELABEL => self.visit_default_case_label(ctx, tree),
            Tag::PARENTHESIZEDPATTERN => self.visit_parenthesized_pattern(ctx, tree),
            Tag::GUARDPATTERN => self.visit_guard_pattern(ctx, tree),
            Tag::INDEXED => self.visit_indexed(ctx, tree),
            Tag::SELECT => self.visit_select(ctx, tree),
            Tag::REFERENCE => self.visit_reference(ctx, tree),
            Tag::IDENT => self.visit_ident(ctx, tree),
            Tag::LITERAL => self.visit_literal(ctx, tree),
            Tag::TYPEIDENT => self.visit_type_ident(ctx, tree),
            Tag::TYPEARRAY => self.visit_type_array(ctx, tree),
            Tag::TYPEAPPLY => self.visit_type_apply(ctx, tree),
            Tag::TYPEUNION => self.visit_type_union(ctx, tree),
            Tag::TYPEINTERSECTION => self.visit_type_intersection(ctx, tree),
            Tag::TYPEPARAMETER => self.visit_type_parameter(ctx, tree),
            Tag::WILDCARD => self.visit_wildcard(ctx, tree),
            Tag::TYPEBOUNDKIND => self.visit_type_bound_kind(ctx, tree),
            Tag::ANNOTATION | Tag::TYPE_ANNOTATION => self.visit_annotation(ctx, tree),
            Tag::MODIFIERS => self.visit_modifiers(ctx, tree),
            Tag::ANNOTATED_TYPE => self.visit_annotated_type(ctx, tree),
            Tag::MODULEDEF => self.visit_module_def(ctx, tree),
            Tag::EXPORTS => self.visit_exports(ctx, tree),
            Tag::OPENS => self.visit_opens(ctx, tree),
            Tag::PROVIDES => self.visit_provides(ctx, tree),
            Tag::REQUIRES => self.visit_requires(ctx, tree),
            Tag::USES => self.visit_uses(ctx, tree),
            Tag::ERRONEOUS => self.visit_erroneous(ctx, tree),
            Tag::LETEXPR => self.visit_let_expr(ctx, tree),
            Tag::NO_TAG => self.visit_tree(ctx, tree),
        }
    }

    /// Java: public void visitSkip(JCSkip tree)
    fn visit_skip(&mut self, _ctx: &CompilerContext, _tree: TreeId) {
        self.print(";");
    }

    /// Java: public void visitDefaultCaseLabel(JCTree.JCDefaultCaseLabel that)
    fn visit_default_case_label(&mut self, _ctx: &CompilerContext, _tree: TreeId) {
        self.print("default");
    }

    /// Java: public void visitErroneous(JCErroneous tree)
    fn visit_erroneous(&mut self, _ctx: &CompilerContext, _tree: TreeId) {
        self.print("(ERROR)");
    }

    fn visit_tree(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let tag = ctx.tree(tree).tag;
        self.print(&format!("(UNKNOWN: {tag:?})"));
        self.println();
    }

    fn visit_top_level(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_unit(ctx, tree, None);
    }

    fn visit_package_def(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_doc_comment(ctx, tree);
        let TreeKind::PackageDecl {
            annotations, pid, ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let annotations = annotations.clone();
        let pid = *pid;
        self.print_annotations(ctx, &annotations);
        if let Some(pid) = pid {
            self.print("package ");
            self.print_expr(ctx, pid);
            self.print(";");
            self.println();
        }
    }

    fn visit_module_def(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_doc_comment(ctx, tree);
        let TreeKind::ModuleDecl {
            mods,
            kind,
            qual_id,
            directives,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let mods = *mods;
        let kind = *kind;
        let qual_id = *qual_id;
        let directives = directives.clone();
        if let Some(mods) = mods {
            if let TreeKind::Modifiers { annotations, .. } = &ctx.tree(mods).kind {
                let annotations = annotations.clone();
                self.print_annotations(ctx, &annotations);
            }
        }
        if kind == javac_source::tree::module_tree::ModuleKind::OPEN {
            self.print("open ");
        }
        self.print("module ");
        self.print_expr_opt(ctx, qual_id);
        match directives {
            None => self.print(";"),
            Some(directives) => {
                self.print(" ");
                self.print_block(ctx, &directives);
            }
        }
        self.println();
    }

    fn visit_exports(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Exports {
            qualid,
            module_names,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let qualid = *qualid;
        let module_names = module_names.clone();
        self.print("exports ");
        self.print_expr_opt(ctx, qualid);
        if let Some(module_names) = module_names {
            self.print(" to ");
            self.print_exprs(ctx, &module_names, ", ");
        }
        self.print(";");
    }

    fn visit_opens(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Opens {
            qualid,
            module_names,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let qualid = *qualid;
        let module_names = module_names.clone();
        self.print("opens ");
        self.print_expr_opt(ctx, qualid);
        if let Some(module_names) = module_names {
            self.print(" to ");
            self.print_exprs(ctx, &module_names, ", ");
        }
        self.print(";");
    }

    fn visit_provides(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Provides {
            service_name,
            impl_names,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let service_name = *service_name;
        let impl_names = impl_names.clone();
        self.print("provides ");
        self.print_expr_opt(ctx, service_name);
        self.print(" with ");
        self.print_exprs(ctx, &impl_names, ", ");
        self.print(";");
    }

    fn visit_requires(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Requires {
            is_transitive,
            is_static_phase,
            module_name,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let is_transitive = *is_transitive;
        let is_static_phase = *is_static_phase;
        let module_name = *module_name;
        self.print("requires ");
        if is_static_phase {
            self.print("static ");
        }
        if is_transitive {
            self.print("transitive ");
        }
        self.print_expr_opt(ctx, module_name);
        self.print(";");
    }

    fn visit_uses(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Uses { qualid } = &ctx.tree(tree).kind else {
            return;
        };
        let qualid = *qualid;
        self.print("uses ");
        self.print_expr_opt(ctx, qualid);
        self.print(";");
    }

    fn visit_import(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Import {
            static_import,
            qualid,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let static_import = *static_import;
        let qualid = *qualid;
        self.print("import ");
        if static_import {
            self.print("static ");
        }
        self.print_expr_opt(ctx, qualid);
        self.print(";");
        self.println();
    }

    fn visit_class_def(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::ClassDecl {
            mods,
            name,
            typarams,
            extending,
            implementing,
            permitting,
            defs,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let mods = *mods;
        let name = name.clone();
        let typarams = typarams.clone();
        let extending = *extending;
        let implementing = implementing.clone();
        let permitting = permitting.clone();
        let defs = defs.clone();
        self.println();
        self.align();
        self.print_doc_comment(ctx, tree);
        let (mod_flags, annotations) = match mods {
            Some(mods) => match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, annotations } => (*flags, annotations.clone()),
                _ => (0, List::nil()),
            },
            None => (0, List::nil()),
        };
        self.print_annotations(ctx, &annotations);
        self.print_flags(mod_flags & !(crate::code::flags::INTERFACE as i64));
        let encl_class_name_prev = self.encl_class_name.clone();
        self.encl_class_name = Some(name.clone());
        if (mod_flags & crate::code::flags::INTERFACE as i64) != 0 {
            self.print("interface ");
            self.print_name(&name);
            self.print_type_parameters(ctx, &typarams);
            if implementing.non_empty() {
                self.print(" extends ");
                self.print_exprs(ctx, &implementing, ", ");
            }
            if permitting.non_empty() {
                self.print(" permits ");
                self.print_exprs(ctx, &permitting, ", ");
            }
        } else {
            if (mod_flags & crate::code::flags::ENUM as i64) != 0 {
                self.print("enum ");
            } else {
                self.print("class ");
            }
            self.print_name(&name);
            self.print_type_parameters(ctx, &typarams);
            if let Some(extending) = extending {
                self.print(" extends ");
                self.print_expr(ctx, extending);
            }
            if implementing.non_empty() {
                self.print(" implements ");
                self.print_exprs(ctx, &implementing, ", ");
            }
            if permitting.non_empty() {
                self.print(" permits ");
                self.print_exprs(ctx, &permitting, ", ");
            }
        }
        self.print(" ");
        if (mod_flags & crate::code::flags::ENUM as i64) != 0 {
            self.print_enum_body(ctx, &defs);
        } else {
            self.print_block(ctx, &defs);
        }
        self.encl_class_name = encl_class_name_prev;
    }

    fn visit_method_def(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::MethodDecl {
            mods,
            name,
            restype,
            typarams,
            recvparam,
            params,
            thrown,
            body,
            default_value,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let mods = *mods;
        let name = name.clone();
        let restype = *restype;
        let typarams = typarams.clone();
        let recvparam = *recvparam;
        let params = params.clone();
        let thrown = thrown.clone();
        let body = *body;
        let default_value = *default_value;
        let names = name.table.names();
        // when producing source output, omit anonymous constructors
        if name.to_java_string() == names.init.to_java_string()
            && self.encl_class_name.is_none()
            && self.source_output
        {
            return;
        }
        self.println();
        self.align();
        self.print_doc_comment(ctx, tree);
        if let Some(mods) = mods {
            self.print_expr(ctx, mods);
        }
        self.print_type_parameters(ctx, &typarams);
        if name.to_java_string() == names.init.to_java_string() {
            let init_name = match &self.encl_class_name {
                Some(encl) => encl.clone(),
                None => name.clone(),
            };
            self.print_name(&init_name);
        } else {
            self.print_expr_opt(ctx, restype);
            self.print(" ");
            self.print_name(&name);
        }
        self.print("(");
        if let Some(recvparam) = recvparam {
            self.print_expr(ctx, recvparam);
            if params.length() > 0 {
                self.print(", ");
            }
        }
        self.print_exprs(ctx, &params, ", ");
        self.print(")");
        if thrown.non_empty() {
            self.print(" throws ");
            self.print_exprs(ctx, &thrown, ", ");
        }
        if let Some(default_value) = default_value {
            self.print(" default ");
            self.print_expr(ctx, default_value);
        }
        if let Some(body) = body {
            self.print(" ");
            self.print_stat(ctx, body);
        } else {
            self.print(";");
        }
    }

    fn visit_var_def(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::VariableDecl {
            mods,
            name,
            vartype,
            init,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let mods = *mods;
        let name = name.clone();
        let vartype = *vartype;
        let init = *init;
        let (mod_flags, _annotations) = match mods {
            Some(mods) => match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, annotations } => (*flags, annotations.clone()),
                _ => (0, List::nil()),
            },
            None => (0, List::nil()),
        };
        if let Some(doc_comments) = self.doc_comments.clone() {
            if doc_comments.borrow().has_comment(tree) {
                self.println();
                self.align();
            }
        }
        self.print_doc_comment(ctx, tree);
        if (mod_flags & crate::code::flags::ENUM as i64) != 0 {
            self.print("/*public static final*/ ");
            self.print_name(&name);
            if let Some(init) = init {
                let init_node = ctx.tree(init);
                if init_node.has_tag(Tag::NEWCLASS) {
                    let TreeKind::NewClass { args, def, .. } = &init_node.kind else {
                        return;
                    };
                    let args = args.clone();
                    let def = *def;
                    if self.source_output {
                        self.print(" /*enum*/ ");
                        if args.non_empty() {
                            self.print("(");
                            for arg in args.iter() {
                                self.print_expr(ctx, arg);
                            }
                            self.print(")");
                        }
                        if let Some(def) = def {
                            if let TreeKind::ClassDecl { defs, .. } = &ctx.tree(def).kind {
                                self.print(" ");
                                self.print_block(ctx, &defs.clone());
                            }
                        }
                        return;
                    } else {
                        self.print(" /* = ");
                        self.print("new ");
                        if let Some(def) = def {
                            if let TreeKind::ClassDecl { mods, .. } = &ctx.tree(def).kind {
                                if let Some(mods) = *mods {
                                    if let TreeKind::Modifiers { annotations, .. } =
                                        &ctx.tree(mods).kind
                                    {
                                        let annotations = annotations.clone();
                                        if annotations.non_empty() {
                                            self.print_type_annotations(ctx, &annotations);
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(def) = def {
                            if let TreeKind::ClassDecl { defs, .. } = &ctx.tree(def).kind {
                                let defs = defs.clone();
                                if let TreeKind::NewClass { clazz, .. } = &ctx.tree(init).kind {
                                    self.print_expr_opt(ctx, *clazz);
                                }
                                self.print("(");
                                self.print_exprs(ctx, &args, ", ");
                                self.print(")");
                                self.print(" */");
                                self.print(" /*enum*/ ");
                                if args.non_empty() {
                                    self.print("(");
                                    self.print_exprs(ctx, &args, ", ");
                                    self.print(")");
                                }
                                self.print(" ");
                                self.print_block(ctx, &defs);
                            }
                        }
                        return;
                    }
                }
                self.print(" /* = ");
                self.print_expr(ctx, init);
                self.print(" */");
            }
        } else {
            if let Some(mods) = mods {
                self.print_expr(ctx, mods);
            }
            let mut vartype = vartype;
            if (mod_flags & crate::code::flags::VARARGS) != 0 {
                let mut tas: Option<List<TreeId>> = None;
                if let Some(vt) = vartype {
                    if let TreeKind::AnnotatedType {
                        annotations,
                        underlying_type,
                    } = &ctx.tree(vt).kind
                    {
                        tas = Some(annotations.clone());
                        vartype = *underlying_type;
                    }
                }
                if let Some(vt) = vartype {
                    if let TreeKind::ArrayTypeTree { elemtype } = &ctx.tree(vt).kind {
                        self.print_expr_opt(ctx, *elemtype);
                    }
                }
                if let Some(tas) = tas {
                    self.print(" ");
                    self.print_type_annotations(ctx, &tas);
                }
                self.print("... ");
                self.print_name(&name);
            } else {
                self.print_expr_opt(ctx, vartype);
                self.print(" ");
                self.print_name(&name);
            }
            if let Some(init) = init {
                self.print(" = ");
                self.print_expr(ctx, init);
            }
            if self.prec == tree_info::NOT_EXPRESSION {
                self.print(";");
            }
        }
    }

    fn visit_block(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Block { flags, stats, .. } = &ctx.tree(tree).kind else {
            return;
        };
        let flags = *flags;
        let stats = stats.clone();
        self.print_flags(flags);
        self.print_block(ctx, &stats);
    }

    fn visit_do_loop(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::DoWhileLoop { body, cond } = &ctx.tree(tree).kind else {
            return;
        };
        let body = *body;
        let cond = *cond;
        self.print("do ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
        self.align();
        self.print(" while ");
        self.print_cond_in_parens(ctx, cond);
        self.print(";");
    }

    fn print_cond_in_parens(&mut self, ctx: &CompilerContext, cond: Option<TreeId>) {
        match cond {
            Some(cond) if ctx.tree(cond).has_tag(Tag::PARENS) => self.print_expr(ctx, cond),
            Some(cond) => {
                self.print("(");
                self.print_expr(ctx, cond);
                self.print(")");
            }
            None => {}
        }
    }

    fn visit_while_loop(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::WhileLoop { cond, body } = &ctx.tree(tree).kind else {
            return;
        };
        let cond = *cond;
        let body = *body;
        self.print("while ");
        self.print_cond_in_parens(ctx, cond);
        self.print(" ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
    }

    fn visit_for_loop(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::ForLoop {
            init,
            cond,
            step,
            body,
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let init = init.clone();
        let cond = *cond;
        let step = step.clone();
        let body = *body;
        self.print("for (");
        if init.non_empty() {
            if ctx.tree(*init.head()).has_tag(Tag::VARDEF) {
                self.print_expr(ctx, *init.head());
                if let TreeKind::VariableDecl {
                    name: vdef_name,
                    init: vdef_init,
                    ..
                } = &ctx.tree(*init.head()).kind
                {
                    let vdef_name = vdef_name.clone();
                    let vdef_init = *vdef_init;
                    if let TreeKind::VariableDecl { .. } = &ctx.tree(*init.head()).kind {
                        // first variable printed above; remaining ones print
                        // ", name = init".
                    }
                    let mut rest = init.tail();
                    while rest.non_empty() {
                        if let TreeKind::VariableDecl {
                            name: next_name,
                            init: next_init,
                            ..
                        } = &ctx.tree(*rest.head()).kind
                        {
                            let next_name = next_name.clone();
                            let next_init = *next_init;
                            self.print(", ");
                            self.print_name(&next_name);
                            if let Some(next_init) = next_init {
                                self.print(" = ");
                                self.print_expr(ctx, next_init);
                            }
                        }
                        rest = rest.tail();
                    }
                    let _ = (vdef_name, vdef_init);
                }
            } else {
                self.print_exprs(ctx, &init, ", ");
            }
        }
        self.print("; ");
        if let Some(cond) = cond {
            self.print_expr(ctx, cond);
        }
        self.print("; ");
        self.print_exprs(ctx, &step, ", ");
        self.print(") ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
    }

    fn visit_foreach_loop(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::EnhancedForLoop { var, expr, body } = &ctx.tree(tree).kind else {
            return;
        };
        let var = *var;
        let expr = *expr;
        let body = *body;
        self.print("for (");
        self.print_expr_opt(ctx, var);
        self.print(" : ");
        self.print_expr_opt(ctx, expr);
        self.print(") ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
    }

    fn visit_labelled(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::LabeledStatement { label, body } = &ctx.tree(tree).kind else {
            return;
        };
        let label = label.clone();
        let body = *body;
        self.print_name(&label);
        self.print(": ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
    }

    fn visit_switch(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Switch {
            selector, cases, ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let selector = *selector;
        let cases = cases.clone();
        self.print("switch ");
        self.print_cond_in_parens(ctx, selector);
        self.print(" {");
        self.println();
        self.print_stats(ctx, &cases);
        self.align();
        self.print("}");
    }

    fn visit_case(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Case {
            case_kind,
            labels,
            stats,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let case_kind = *case_kind;
        let labels = labels.clone();
        let stats = stats.clone();
        if labels.length() == 1 && ctx.tree(*labels.head()).has_tag(Tag::DEFAULTCASELABEL) {
            self.print("default");
        } else {
            self.print("case ");
            self.print_exprs(ctx, &labels, ", ");
        }
        if case_kind == javac_source::tree::case_tree::CaseKind::STATEMENT {
            self.print(":");
            self.println();
            self.indent();
            self.print_stats(ctx, &stats);
            self.undent();
            self.align();
        } else {
            self.print(" -> ");
            if stats.length() == 1 {
                self.print_stat(ctx, *stats.head());
            } else {
                self.print_block(ctx, &stats);
            }
        }
    }

    fn visit_switch_expression(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::SwitchExpression {
            selector, cases, ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let selector = *selector;
        let cases = cases.clone();
        self.print("switch ");
        self.print_cond_in_parens(ctx, selector);
        self.print(" {");
        self.println();
        self.print_stats(ctx, &cases);
        self.align();
        self.print("}");
    }

    fn visit_binding_pattern(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::BindingPattern { var } = &ctx.tree(tree).kind {
            self.print_expr_opt(ctx, *var);
        }
    }

    fn visit_parenthesized_pattern(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::ParenthesizedPattern { pattern } = &ctx.tree(tree).kind {
            let pattern = *pattern;
            self.print("(");
            self.print_expr_opt(ctx, pattern);
            self.print(")");
        }
    }

    fn visit_guard_pattern(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::GuardPattern { patt, expr } = &ctx.tree(tree).kind {
            let patt = *patt;
            let expr = *expr;
            self.print_expr_opt(ctx, patt);
            self.print(" && ");
            self.print_expr_opt(ctx, expr);
        }
    }

    fn visit_synchronized(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Synchronized { lock, body } = &ctx.tree(tree).kind else {
            return;
        };
        let lock = *lock;
        let body = *body;
        self.print("synchronized ");
        self.print_cond_in_parens(ctx, lock);
        self.print(" ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
    }

    fn visit_try(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Try {
            body,
            catchers,
            finalizer,
            resources,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let body = *body;
        let catchers = catchers.clone();
        let finalizer = *finalizer;
        let resources = resources.clone();
        self.print("try ");
        if resources.non_empty() {
            self.print("(");
            let mut first = true;
            for var in resources.iter() {
                if !first {
                    self.println();
                    self.indent();
                }
                self.print_stat(ctx, var);
                first = false;
            }
            self.print(") ");
        }
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
        for catcher in catchers.iter() {
            self.print_stat(ctx, catcher);
        }
        if let Some(finalizer) = finalizer {
            self.print(" finally ");
            self.print_stat(ctx, finalizer);
        }
    }

    fn visit_catch(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Catch { param, body } = &ctx.tree(tree).kind else {
            return;
        };
        let param = *param;
        let body = *body;
        self.print(" catch (");
        self.print_expr_opt(ctx, param);
        self.print(") ");
        if let Some(body) = body {
            self.print_stat(ctx, body);
        }
    }

    fn visit_conditional(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Conditional {
            cond,
            truepart,
            falsepart,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let cond = *cond;
        let truepart = *truepart;
        let falsepart = *falsepart;
        let prec = self.prec;
        self.open(prec, tree_info::COND_PREC);
        if let Some(cond) = cond {
            self.print_expr_prec(ctx, cond, tree_info::COND_PREC + 1);
        }
        self.print(" ? ");
        self.print_expr_opt(ctx, truepart);
        self.print(" : ");
        if let Some(falsepart) = falsepart {
            self.print_expr_prec(ctx, falsepart, tree_info::COND_PREC);
        }
        self.close(prec, tree_info::COND_PREC);
    }

    fn visit_if(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::If {
            cond,
            thenpart,
            elsepart,
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let cond = *cond;
        let thenpart = *thenpart;
        let elsepart = *elsepart;
        self.print("if ");
        self.print_cond_in_parens(ctx, cond);
        self.print(" ");
        if let Some(thenpart) = thenpart {
            self.print_stat(ctx, thenpart);
        }
        if let Some(elsepart) = elsepart {
            self.print(" else ");
            self.print_stat(ctx, elsepart);
        }
    }

    fn visit_exec(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::ExpressionStatement { expr } = &ctx.tree(tree).kind else {
            return;
        };
        let expr = *expr;
        self.print_expr_opt(ctx, expr);
        if self.prec == tree_info::NOT_EXPRESSION {
            self.print(";");
        }
    }

    fn visit_break(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Break { label, .. } = &ctx.tree(tree).kind else {
            return;
        };
        let label = label.clone();
        self.print("break");
        if let Some(label) = label {
            self.print(" ");
            self.print_name(&label);
        }
        self.print(";");
    }

    fn visit_yield(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Yield { value, .. } = &ctx.tree(tree).kind else {
            return;
        };
        let value = *value;
        self.print("yield");
        self.print(" ");
        self.print_expr_opt(ctx, value);
        self.print(";");
    }

    fn visit_continue(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Continue { label, .. } = &ctx.tree(tree).kind else {
            return;
        };
        let label = label.clone();
        self.print("continue");
        if let Some(label) = label {
            self.print(" ");
            self.print_name(&label);
        }
        self.print(";");
    }

    fn visit_return(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Return { expr } = &ctx.tree(tree).kind else {
            return;
        };
        let expr = *expr;
        self.print("return");
        if let Some(expr) = expr {
            self.print(" ");
            self.print_expr(ctx, expr);
        }
        self.print(";");
    }

    fn visit_throw(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::Throw { expr } = &ctx.tree(tree).kind {
            let expr = *expr;
            self.print("throw ");
            self.print_expr_opt(ctx, expr);
            self.print(";");
        }
    }

    fn visit_assert(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Assert { cond, detail } = &ctx.tree(tree).kind else {
            return;
        };
        let cond = *cond;
        let detail = *detail;
        self.print("assert ");
        self.print_expr_opt(ctx, cond);
        if let Some(detail) = detail {
            self.print(" : ");
            self.print_expr(ctx, detail);
        }
        self.print(";");
    }

    fn visit_apply(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::MethodInvocation {
            typeargs,
            meth,
            args,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let typeargs = typeargs.clone();
        let meth = *meth;
        let args = args.clone();
        if typeargs.non_empty() {
            let meth_is_select = meth
                .map(|m| ctx.tree(m).has_tag(Tag::SELECT))
                .unwrap_or(false);
            if meth_is_select {
                let TreeKind::FieldAccess { selected, name, .. } =
                    &ctx.tree(meth.expect("apply without method")).kind
                else {
                    return;
                };
                let selected = *selected;
                let name = name.clone();
                self.print_expr_opt(ctx, selected);
                self.print(".<");
                self.print_exprs(ctx, &typeargs, ", ");
                self.print(">");
                self.print_name(&name);
            } else {
                self.print("<");
                self.print_exprs(ctx, &typeargs, ", ");
                self.print(">");
                self.print_expr_opt(ctx, meth);
            }
        } else {
            self.print_expr_opt(ctx, meth);
        }
        self.print("(");
        self.print_exprs(ctx, &args, ", ");
        self.print(")");
    }

    fn visit_new_class(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::NewClass {
            encl,
            typeargs,
            clazz,
            args,
            def,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let encl = *encl;
        let typeargs = typeargs.clone();
        let clazz = *clazz;
        let args = args.clone();
        let def = *def;
        if let Some(encl) = encl {
            self.print_expr(ctx, encl);
            self.print(".");
        }
        self.print("new ");
        if typeargs.non_empty() {
            self.print("<");
            self.print_exprs(ctx, &typeargs, ", ");
            self.print(">");
        }
        if let Some(def) = def {
            if let TreeKind::ClassDecl { mods, .. } = &ctx.tree(def).kind {
                if let Some(mods) = *mods {
                    if let TreeKind::Modifiers { annotations, .. } = &ctx.tree(mods).kind {
                        let annotations = annotations.clone();
                        if annotations.non_empty() {
                            self.print_type_annotations(ctx, &annotations);
                        }
                    }
                }
            }
        }
        self.print_expr_opt(ctx, clazz);
        self.print("(");
        self.print_exprs(ctx, &args, ", ");
        self.print(")");
        if let Some(def) = def {
            let encl_class_name_prev = self.encl_class_name.clone();
            self.encl_class_name = match &ctx.tree(def).kind {
                TreeKind::ClassDecl { name, .. } => Some(name.clone()),
                _ => None,
            };
            if let TreeKind::ClassDecl { mods, defs, .. } = &ctx.tree(def).kind {
                let mod_flags = mods
                    .map(|mods| match &ctx.tree(mods).kind {
                        TreeKind::Modifiers { flags, .. } => *flags,
                        _ => 0,
                    })
                    .unwrap_or(0);
                let defs = defs.clone();
                if (mod_flags & crate::code::flags::ENUM as i64) != 0 {
                    self.print("/*enum*/");
                }
                self.print_block(ctx, &defs);
            }
            self.encl_class_name = encl_class_name_prev;
        }
    }

    fn visit_new_array(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::NewArray {
            elemtype,
            dims,
            annotations,
            dim_annotations,
            elems,
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let elemtype = *elemtype;
        let dims = dims.clone();
        let annotations = annotations.clone();
        let dim_annotations = dim_annotations.clone();
        let elems = elems.clone();
        if let Some(elemtype) = elemtype {
            self.print("new ");
            self.print_base_element_type(ctx, elemtype);
            if !annotations.is_empty() {
                self.print(" ");
                self.print_type_annotations(ctx, &annotations);
            }
            if elems.is_some() {
                self.print("[]");
            }
            for (i, dim) in dims.iter().enumerate() {
                if dim_annotations.length() > i as i32 {
                    let da = dim_annotations.get(i as i32);
                    if !da.is_empty() {
                        self.print(" ");
                        self.print_type_annotations(ctx, &da);
                    }
                }
                self.print("[");
                self.print_expr(ctx, dim);
                self.print("]");
            }
            self.print_brackets(ctx, elemtype);
        }
        if let Some(elems) = elems {
            self.print("{");
            self.print_exprs(ctx, &elems, ", ");
            self.print("}");
        }
    }

    fn visit_lambda(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Lambda {
            params,
            body,
            param_kind,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let params = params.clone();
        let body = *body;
        let param_kind = *param_kind;
        self.print("(");
        if param_kind == crate::tree::jc_tree::ParameterKind::EXPLICIT {
            self.print_exprs(ctx, &params, ", ");
        } else {
            let mut sep = "";
            for param in params.iter() {
                self.print(sep);
                if let TreeKind::VariableDecl { name, .. } = &ctx.tree(param).kind {
                    self.print_name(name);
                }
                sep = ",";
            }
        }
        self.print(")->");
        self.print_expr_opt(ctx, body);
    }

    fn visit_parens(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::Parens { expr } = &ctx.tree(tree).kind {
            let expr = *expr;
            self.print("(");
            self.print_expr_opt(ctx, expr);
            self.print(")");
        }
    }

    fn visit_assign(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::Assign { lhs, rhs } = &ctx.tree(tree).kind {
            let lhs = *lhs;
            let rhs = *rhs;
            let prec = self.prec;
            self.open(prec, tree_info::ASSIGN_PREC);
            if let Some(lhs) = lhs {
                self.print_expr_prec(ctx, lhs, tree_info::ASSIGN_PREC + 1);
            }
            self.print(" = ");
            if let Some(rhs) = rhs {
                self.print_expr_prec(ctx, rhs, tree_info::ASSIGN_PREC);
            }
            self.close(prec, tree_info::ASSIGN_PREC);
        }
    }

    /// Java: public String operatorName(JCTree.Tag tag)
    pub fn operator_name(tag: Tag) -> &'static str {
        match tag {
            Tag::POS => "+",
            Tag::NEG => "-",
            Tag::NOT => "!",
            Tag::COMPL => "~",
            Tag::PREINC => "++",
            Tag::PREDEC => "--",
            Tag::POSTINC => "++",
            Tag::POSTDEC => "--",
            Tag::NULLCHK => "<*nullchk*>",
            Tag::OR => "||",
            Tag::AND => "&&",
            Tag::EQ => "==",
            Tag::NE => "!=",
            Tag::LT => "<",
            Tag::GT => ">",
            Tag::LE => "<=",
            Tag::GE => ">=",
            Tag::BITOR => "|",
            Tag::BITXOR => "^",
            Tag::BITAND => "&",
            Tag::SL => "<<",
            Tag::SR => ">>",
            Tag::USR => ">>>",
            Tag::PLUS => "+",
            Tag::MINUS => "-",
            Tag::MUL => "*",
            Tag::DIV => "/",
            Tag::MOD => "%",
            _ => panic!("unexpected operator tag"),
        }
    }

    fn visit_assignop(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::AssignOp { lhs, rhs, .. } = &ctx.tree(tree).kind else {
            return;
        };
        let lhs = *lhs;
        let rhs = *rhs;
        let tag = ctx.tree(tree).tag;
        let prec = self.prec;
        self.open(prec, tree_info::ASSIGNOP_PREC);
        if let Some(lhs) = lhs {
            self.print_expr_prec(ctx, lhs, tree_info::ASSIGNOP_PREC + 1);
        }
        self.print(" ");
        self.print(Pretty::operator_name(tag.no_assign_op()));
        self.print("= ");
        if let Some(rhs) = rhs {
            self.print_expr_prec(ctx, rhs, tree_info::ASSIGNOP_PREC);
        }
        self.close(prec, tree_info::ASSIGNOP_PREC);
    }

    fn visit_unary(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).tag;
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => None,
        };
        let ownprec = tree_info::op_prec(tree_kind);
        let opname = Pretty::operator_name(tree_kind);
        let prec = self.prec;
        self.open(prec, ownprec);
        if !tree_kind.is_post_unary_op() {
            self.print(opname);
            if let Some(arg) = arg {
                self.print_expr_prec(ctx, arg, ownprec);
            }
        } else {
            if let Some(arg) = arg {
                self.print_expr_prec(ctx, arg, ownprec);
            }
            self.print(opname);
        }
        self.close(prec, ownprec);
    }

    fn visit_binary(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).tag;
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => (None, None),
        };
        let ownprec = tree_info::op_prec(tree_kind);
        let opname = Pretty::operator_name(tree_kind);
        let prec = self.prec;
        self.open(prec, ownprec);
        if let Some(lhs) = lhs {
            self.print_expr_prec(ctx, lhs, ownprec);
        }
        self.print(" ");
        self.print(opname);
        self.print(" ");
        if let Some(rhs) = rhs {
            self.print_expr_prec(ctx, rhs, ownprec + 1);
        }
        self.close(prec, ownprec);
    }

    fn visit_type_cast(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::TypeCast { clazz, expr } = &ctx.tree(tree).kind {
            let clazz = *clazz;
            let expr = *expr;
            let prec = self.prec;
            self.open(prec, tree_info::PREFIX_PREC);
            self.print("(");
            self.print_expr_opt(ctx, clazz);
            self.print(")");
            if let Some(expr) = expr {
                self.print_expr_prec(ctx, expr, tree_info::PREFIX_PREC);
            }
            self.close(prec, tree_info::PREFIX_PREC);
        }
    }

    fn visit_type_test(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::InstanceOf { expr, pattern } = &ctx.tree(tree).kind {
            let expr = *expr;
            let pattern = *pattern;
            let prec = self.prec;
            self.open(prec, tree_info::ORD_PREC);
            if let Some(expr) = expr {
                self.print_expr_prec(ctx, expr, tree_info::ORD_PREC);
            }
            self.print(" instanceof ");
            if let Some(pattern) = pattern {
                self.print_pattern(ctx, pattern);
            }
            self.close(prec, tree_info::ORD_PREC);
        }
    }

    fn visit_indexed(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::ArrayAccess { indexed, index } = &ctx.tree(tree).kind {
            let indexed = *indexed;
            let index = *index;
            if let Some(indexed) = indexed {
                self.print_expr_prec(ctx, indexed, tree_info::POSTFIX_PREC);
            }
            self.print("[");
            self.print_expr_opt(ctx, index);
            self.print("]");
        }
    }

    fn visit_select(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::FieldAccess { selected, name, .. } = &ctx.tree(tree).kind {
            let selected = *selected;
            let name = name.clone();
            if let Some(selected) = selected {
                self.print_expr_prec(ctx, selected, tree_info::POSTFIX_PREC);
            }
            self.print(".");
            self.print_name(&name);
        }
    }

    fn visit_reference(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::MemberReference {
            mode,
            name,
            expr,
            typeargs,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let mode = *mode;
        let name = name.clone();
        let expr = *expr;
        let typeargs = typeargs.clone();
        self.print_expr_opt(ctx, expr);
        self.print("::");
        if let Some(typeargs) = typeargs {
            self.print("<");
            self.print_exprs(ctx, &typeargs, ", ");
            self.print(">");
        }
        match mode {
            javac_source::tree::member_reference_tree::ReferenceMode::INVOKE => {
                self.print_name(&name);
            }
            javac_source::tree::member_reference_tree::ReferenceMode::NEW => self.print("new"),
        }
    }

    fn visit_ident(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::Ident { name, .. } = &ctx.tree(tree).kind {
            self.print_name(name);
        }
    }

    fn visit_literal(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Literal { typetag, value } = &ctx.tree(tree).kind else {
            return;
        };
        let typetag = *typetag;
        let value = value.clone();
        match typetag {
            TypeTag::INT => match value {
                Some(LiteralValue::Integer(v)) => self.print(&v.to_string()),
                _ => self.print("0"),
            },
            TypeTag::LONG => match value {
                Some(LiteralValue::Long(v)) => self.print(&format!("{v}L")),
                _ => self.print("0L"),
            },
            TypeTag::FLOAT => match value {
                // Java: print(tree.value + "F") -> Float.toString(v) + "F"
                // (FloatingDecimal.toJavaFormatString; javac-floating).
                Some(LiteralValue::Float(v)) => {
                    self.print(&format!("{}F", javac_floating::to_java_string_float(v)))
                }
                _ => self.print("0.0F"),
            },
            TypeTag::DOUBLE => match value {
                // Java: print(tree.value.toString()) -> Double.toString(v)
                // (FloatingDecimal.toJavaFormatString; javac-floating).
                Some(LiteralValue::Double(v)) => {
                    self.print(&javac_floating::to_java_string_double(v))
                }
                _ => self.print("0.0"),
            },
            TypeTag::CHAR => match value {
                Some(LiteralValue::Character(v)) => {
                    let text = JavaString::from_units(vec![v]);
                    self.print("'");
                    self.print_java_string(&convert::quote(&text));
                    self.print("'");
                }
                _ => self.print("' '"),
            },
            TypeTag::BOOLEAN => match value {
                Some(LiteralValue::Boolean(v)) => self.print(if v { "true" } else { "false" }),
                _ => self.print("false"),
            },
            TypeTag::BOT => self.print("null"),
            _ => match value {
                Some(LiteralValue::String(v)) => {
                    self.print("\"");
                    self.print_java_string(&convert::quote(&v));
                    self.print("\"");
                }
                // Java: Pretty.java:1427-1428 (the default branch prints the
                // value with quotes, so BYTE/SHORT print as "n").
                Some(LiteralValue::Byte(v)) => {
                    self.print(&format!("\"{}\"", v));
                }
                Some(LiteralValue::Short(v)) => {
                    self.print(&format!("\"{}\"", v));
                }
                _ => self.print("\"\""),
            },
        }
    }

    fn visit_type_ident(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::PrimitiveTypeTree { typetag } = &ctx.tree(tree).kind else {
            return;
        };
        self.print(match typetag {
            TypeTag::BYTE => "byte",
            TypeTag::CHAR => "char",
            TypeTag::SHORT => "short",
            TypeTag::INT => "int",
            TypeTag::LONG => "long",
            TypeTag::FLOAT => "float",
            TypeTag::DOUBLE => "double",
            TypeTag::BOOLEAN => "boolean",
            TypeTag::VOID => "void",
            _ => "error",
        });
    }

    fn visit_type_array(&mut self, ctx: &CompilerContext, tree: TreeId) {
        self.print_base_element_type(ctx, tree);
        self.print_brackets(ctx, tree);
    }

    /// Prints the inner element type of a nested array.
    fn print_base_element_type(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let innermost = tree_info::innermost_type(ctx, tree, false).expect("type has no innermost");
        self.print_expr(ctx, innermost);
    }

    /// Prints the brackets of a nested array in reverse order.
    fn print_brackets(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let mut elem = tree;
        loop {
            if ctx.tree(elem).has_tag(Tag::ANNOTATED_TYPE) {
                let (annotations, underlying) = match &ctx.tree(elem).kind {
                    TreeKind::AnnotatedType {
                        annotations,
                        underlying_type,
                    } => (annotations.clone(), *underlying_type),
                    _ => (List::nil(), None),
                };
                if let Some(underlying) = underlying {
                    elem = underlying;
                    if ctx.tree(elem).has_tag(Tag::TYPEARRAY) {
                        self.print(" ");
                        self.print_type_annotations(ctx, &annotations);
                    }
                } else {
                    break;
                }
            }
            if ctx.tree(elem).has_tag(Tag::TYPEARRAY) {
                self.print("[]");
                match &ctx.tree(elem).kind {
                    TreeKind::ArrayTypeTree { elemtype } => match elemtype {
                        Some(elemtype) => elem = *elemtype,
                        None => break,
                    },
                    _ => break,
                }
            } else {
                break;
            }
        }
    }

    fn visit_type_apply(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::TypeApply { clazz, arguments } = &ctx.tree(tree).kind {
            let clazz = *clazz;
            let arguments = arguments.clone();
            self.print_expr_opt(ctx, clazz);
            self.print("<");
            self.print_exprs(ctx, &arguments, ", ");
            self.print(">");
        }
    }

    fn visit_type_union(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::TypeUnion { alternatives } = &ctx.tree(tree).kind {
            let alternatives = alternatives.clone();
            self.print_exprs(ctx, &alternatives, " | ");
        }
    }

    fn visit_type_intersection(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::TypeIntersection { bounds } = &ctx.tree(tree).kind {
            let bounds = bounds.clone();
            self.print_exprs(ctx, &bounds, " & ");
        }
    }

    fn visit_type_parameter(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::TypeParameter {
            name,
            bounds,
            annotations,
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let name = name.clone();
        let bounds = bounds.clone();
        let annotations = annotations.clone();
        if annotations.non_empty() {
            self.print_type_annotations(ctx, &annotations);
        }
        self.print_name(&name);
        if bounds.non_empty() {
            self.print(" extends ");
            self.print_exprs(ctx, &bounds, " & ");
        }
    }

    fn visit_wildcard(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Wildcard { kind, inner } = &ctx.tree(tree).kind else {
            return;
        };
        let kind = *kind;
        let inner = *inner;
        if let Some(kind) = kind {
            self.print_expr(ctx, kind);
            let bound_kind = match &ctx.tree(kind).kind {
                TreeKind::TypeBoundKind { kind } => *kind,
                _ => BoundKind::UNBOUND,
            };
            if bound_kind != BoundKind::UNBOUND {
                self.print_expr_opt(ctx, inner);
            }
        }
    }

    fn visit_type_bound_kind(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::TypeBoundKind { kind } = &ctx.tree(tree).kind {
            self.print(kind.name());
        }
    }

    fn visit_let_expr(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::LetExpr { defs, expr, .. } = &ctx.tree(tree).kind else {
            return;
        };
        let defs = defs.clone();
        let expr = *expr;
        self.print("(let ");
        self.print_exprs(ctx, &defs, ", ");
        self.print(" in ");
        self.print_expr_opt(ctx, expr);
        self.print(")");
    }

    fn visit_modifiers(&mut self, ctx: &CompilerContext, tree: TreeId) {
        if let TreeKind::Modifiers { flags, annotations } = &ctx.tree(tree).kind {
            let flags = *flags;
            let annotations = annotations.clone();
            self.print_annotations(ctx, &annotations);
            self.print_flags(flags);
        }
    }

    fn visit_annotation(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::Annotation {
            annotation_type,
            args,
            ..
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let annotation_type = *annotation_type;
        let args = args.clone();
        self.print("@");
        self.print_expr_opt(ctx, annotation_type);
        if !args.is_empty() {
            self.print("(");
            self.print_exprs(ctx, &args, ", ");
            self.print(")");
        }
    }

    fn visit_annotated_type(&mut self, ctx: &CompilerContext, tree: TreeId) {
        let TreeKind::AnnotatedType {
            annotations,
            underlying_type,
        } = &ctx.tree(tree).kind
        else {
            return;
        };
        let annotations = annotations.clone();
        let underlying_type = *underlying_type;
        let Some(underlying_type) = underlying_type else {
            return;
        };
        if ctx.tree(underlying_type).has_tag(Tag::SELECT) {
            if let TreeKind::FieldAccess { selected, name, .. } = &ctx.tree(underlying_type).kind {
                let selected = *selected;
                let name = name.clone();
                if let Some(selected) = selected {
                    self.print_expr_prec(ctx, selected, tree_info::POSTFIX_PREC);
                }
                self.print(".");
                self.print_type_annotations(ctx, &annotations);
                self.print_name(&name);
            }
        } else if ctx.tree(underlying_type).has_tag(Tag::TYPEARRAY) {
            self.print_base_element_type(ctx, tree);
            self.print_brackets(ctx, tree);
        } else {
            self.print_type_annotations(ctx, &annotations);
            self.print_expr(ctx, underlying_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::jc_tree::{JCTree, PolyKind};
    use crate::util::name::NameTableHandle;
    use std::rc::Rc;

    fn name(table: &Rc<NameTableHandle>, text: &str) -> Name {
        let units: Vec<u16> = text.encode_utf16().collect();
        table.from_chars(&units, 0, units.len() as i32)
    }

    fn tree(ctx: &mut CompilerContext, tag: Tag, kind: TreeKind, pos: i32) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos,
            ty: None,
            tag,
            kind,
        })
    }

    fn empty() -> List<TreeId> {
        List::nil()
    }

    fn print_tree(ctx: &mut CompilerContext, tree: TreeId) -> String {
        let mut pretty = Pretty::new(false);
        pretty.print_expr(ctx, tree);
        pretty.get_out().to_string_lossy()
    }

    fn ident(ctx: &mut CompilerContext, table: &Rc<NameTableHandle>, text: &str) -> TreeId {
        tree(
            ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: name(table, text),
                sym: None,
            },
            0,
        )
    }

    fn modifiers(ctx: &mut CompilerContext, flags: i64) -> TreeId {
        tree(
            ctx,
            Tag::MODIFIERS,
            TreeKind::Modifiers {
                flags,
                annotations: empty(),
            },
            0,
        )
    }

    #[test]
    fn prints_identifiers_and_selects() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let select = tree(
            &mut ctx,
            Tag::SELECT,
            TreeKind::FieldAccess {
                selected: Some(a),
                name: name(&table, "b"),
                sym: None,
            },
            0,
        );
        assert_eq!(print_tree(&mut ctx, a), "a");
        assert_eq!(print_tree(&mut ctx, select), "a.b");
    }

    #[test]
    fn prints_binary_precedence_with_parens() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let c = ident(&mut ctx, &table, "c");
        let mul = tree(
            &mut ctx,
            Tag::MUL,
            TreeKind::Binary {
                lhs: Some(a),
                rhs: Some(b),
                operator: None,
            },
            0,
        );
        let plus = tree(
            &mut ctx,
            Tag::PLUS,
            TreeKind::Binary {
                lhs: Some(a),
                rhs: Some(b),
                operator: None,
            },
            0,
        );
        assert_eq!(print_tree(&mut ctx, mul), "a * b");
        assert_eq!(print_tree(&mut ctx, plus), "a + b");
        let nested = tree(
            &mut ctx,
            Tag::MUL,
            TreeKind::Binary {
                lhs: Some(plus),
                rhs: Some(c),
                operator: None,
            },
            0,
        );
        // A lower-precedence operand is parenthesized (upstream Pretty.open).
        assert_eq!(print_tree(&mut ctx, nested), "(a + b) * c");
    }

    #[test]
    fn prints_class_and_blocks() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let mods = modifiers(&mut ctx, crate::code::flags::PUBLIC as i64);
        let field_mods = modifiers(&mut ctx, crate::code::flags::PRIVATE as i64);
        let int_type = tree(
            &mut ctx,
            Tag::TYPEIDENT,
            TreeKind::PrimitiveTypeTree {
                typetag: TypeTag::INT,
            },
            0,
        );
        let one = tree(
            &mut ctx,
            Tag::LITERAL,
            TreeKind::Literal {
                typetag: TypeTag::INT,
                value: Some(LiteralValue::Integer(1)),
            },
            0,
        );
        let field = tree(
            &mut ctx,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: Some(field_mods),
                name: name(&table, "x"),
                nameexpr: None,
                vartype: Some(int_type),
                init: Some(one),
                sym: None,
                start_pos: crate::util::position::NOPOS,
                declared_using_var: false,
            },
            0,
        );
        let class = tree(
            &mut ctx,
            Tag::CLASSDEF,
            TreeKind::ClassDecl {
                mods: Some(mods),
                name: name(&table, "C"),
                typarams: empty(),
                extending: None,
                implementing: empty(),
                permitting: empty(),
                defs: List::of(field),
                sym: None,
            },
            0,
        );
        let output = print_tree(&mut ctx, class);
        // visitClassDef starts with a println like the upstream visitor.
        assert_eq!(output, "\npublic class C {\n    private int x = 1;\n}");
    }

    #[test]
    fn prints_apply_and_lambda() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let method = ident(&mut ctx, &table, "m");
        let one = tree(
            &mut ctx,
            Tag::LITERAL,
            TreeKind::Literal {
                typetag: TypeTag::INT,
                value: Some(LiteralValue::Integer(1)),
            },
            0,
        );
        let apply = tree(
            &mut ctx,
            Tag::APPLY,
            TreeKind::MethodInvocation {
                typeargs: empty(),
                meth: Some(method),
                args: List::of(one),
                varargs_element: None,
                poly_kind: PolyKind::STANDALONE,
            },
            0,
        );
        assert_eq!(print_tree(&mut ctx, apply), "m(1)");
    }

    #[test]
    fn prints_statements_and_control_flow() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let cond = ident(&mut ctx, &table, "c");
        let call = {
            let method = ident(&mut ctx, &table, "m");
            tree(
                &mut ctx,
                Tag::APPLY,
                TreeKind::MethodInvocation {
                    typeargs: empty(),
                    meth: Some(method),
                    args: empty(),
                    varargs_element: None,
                    poly_kind: PolyKind::STANDALONE,
                },
                0,
            )
        };
        let exec = tree(
            &mut ctx,
            Tag::EXEC,
            TreeKind::ExpressionStatement { expr: Some(call) },
            0,
        );
        let ret = tree(&mut ctx, Tag::RETURN, TreeKind::Return { expr: None }, 0);
        let block = tree(
            &mut ctx,
            Tag::BLOCK,
            TreeKind::Block {
                flags: 0,
                stats: List::of2(exec, ret),
                endpos: crate::util::position::NOPOS,
            },
            0,
        );
        let if_tree = tree(
            &mut ctx,
            Tag::IF,
            TreeKind::If {
                cond: Some(cond),
                thenpart: Some(block),
                elsepart: None,
            },
            0,
        );
        assert_eq!(
            print_tree(&mut ctx, if_tree),
            "if (c) {\n    m();\n    return;\n}"
        );
    }

    #[test]
    fn prints_try_catch() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let try_block = tree(
            &mut ctx,
            Tag::BLOCK,
            TreeKind::Block {
                flags: 0,
                stats: empty(),
                endpos: crate::util::position::NOPOS,
            },
            0,
        );
        let catch_block = tree(
            &mut ctx,
            Tag::BLOCK,
            TreeKind::Block {
                flags: 0,
                stats: empty(),
                endpos: crate::util::position::NOPOS,
            },
            0,
        );
        let exception_type = ident(&mut ctx, &table, "E");
        let param_mods = modifiers(&mut ctx, 0);
        let param = tree(
            &mut ctx,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: Some(param_mods),
                name: name(&table, "e"),
                nameexpr: None,
                vartype: Some(exception_type),
                init: None,
                sym: None,
                start_pos: crate::util::position::NOPOS,
                declared_using_var: false,
            },
            0,
        );
        let catch = tree(
            &mut ctx,
            Tag::CATCH,
            TreeKind::Catch {
                param: Some(param),
                body: Some(catch_block),
            },
            0,
        );
        let try_tree = tree(
            &mut ctx,
            Tag::TRY,
            TreeKind::Try {
                body: Some(try_block),
                catchers: List::of(catch),
                finalizer: None,
                resources: empty(),
                finally_can_complete_normally: true,
            },
            0,
        );
        assert_eq!(print_tree(&mut ctx, try_tree), "try {\n} catch (E e) {\n}");
    }

    #[test]
    fn prints_types_and_new_array() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let e_type = ident(&mut ctx, &table, "E");
        let array = tree(
            &mut ctx,
            Tag::TYPEARRAY,
            TreeKind::ArrayTypeTree {
                elemtype: Some(e_type),
            },
            0,
        );
        assert_eq!(print_tree(&mut ctx, array), "E[]");
        let new_array = tree(
            &mut ctx,
            Tag::NEWARRAY,
            TreeKind::NewArray {
                elemtype: Some(array),
                dims: empty(),
                annotations: empty(),
                dim_annotations: List::nil(),
                elems: None,
            },
            0,
        );
        assert_eq!(print_tree(&mut ctx, new_array), "new E[]");
    }

    #[test]
    fn to_simple_string_flattens_whitespace() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let plus = tree(
            &mut ctx,
            Tag::PLUS,
            TreeKind::Binary {
                lhs: Some(a),
                rhs: Some(b),
                operator: None,
            },
            0,
        );
        assert_eq!(
            Pretty::to_simple_string(&ctx, plus).to_string_lossy(),
            "a + b"
        );
    }

    #[test]
    fn prints_literals() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let make = |ctx: &mut CompilerContext, tag: TypeTag, value: LiteralValue| {
            tree(
                ctx,
                Tag::LITERAL,
                TreeKind::Literal {
                    typetag: tag,
                    value: Some(value),
                },
                0,
            )
        };
        let int_literal = make(&mut ctx, TypeTag::INT, LiteralValue::Integer(42));
        assert_eq!(print_tree(&mut ctx, int_literal), "42");
        let long_literal = make(&mut ctx, TypeTag::LONG, LiteralValue::Long(7));
        assert_eq!(print_tree(&mut ctx, long_literal), "7L");
        let bool_literal = make(&mut ctx, TypeTag::BOOLEAN, LiteralValue::Boolean(true));
        assert_eq!(print_tree(&mut ctx, bool_literal), "true");
        let string_literal = make(
            &mut ctx,
            TypeTag::CLASS,
            LiteralValue::String(JavaString::from("s")),
        );
        assert_eq!(print_tree(&mut ctx, string_literal), "\"s\"");
        let char_literal = make(&mut ctx, TypeTag::CHAR, LiteralValue::Character('a' as u16));
        assert_eq!(print_tree(&mut ctx, char_literal), "'a'");
        // Oracle (JDK 17 Pretty.toSimpleString): BYTE/SHORT literals print in
        // the default branch with quotes: Byte 5 -> "5", Short 7 -> "7".
        let byte_literal = make(&mut ctx, TypeTag::BYTE, LiteralValue::Byte(5));
        assert_eq!(print_tree(&mut ctx, byte_literal), "\"5\"");
        let short_literal = make(&mut ctx, TypeTag::SHORT, LiteralValue::Short(7));
        assert_eq!(print_tree(&mut ctx, short_literal), "\"7\"");
    }
}
