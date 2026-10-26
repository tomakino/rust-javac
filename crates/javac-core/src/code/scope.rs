// Origin: com/sun/tools/javac/code/Scope.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check or ClassReader; see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: isStaticallyImported (Scope.java:162)
// - NOT PORTED: create (Scope.java:268)
// - NOT PORTED: hasNext (Scope.java:581)
// - NOT PORTED: doNext (Scope.java:598)
// - NOT PORTED: skipToNextMatchingEntry (Scope.java:619)
// - NOT PORTED: hasNext (Scope.java:635)
// - NOT PORTED: doNext (Scope.java:651)
// - NOT PORTED: isStaticallyImported (Scope.java:672)
// - NOT PORTED: toString (Scope.java:676)
// - NOT PORTED: finalizeScope (Scope.java:749)
// - NOT PORTED: finalizeSingleScope (Scope.java:755)
// - NOT PORTED: importByName (Scope.java:799)
// - NOT PORTED: importType (Scope.java:803)
// - NOT PORTED: appendScope (Scope.java:807)
// - NOT PORTED: finalizeScope (Scope.java:830)
// - NOT PORTED: isStaticallyImported (Scope.java:871)
// - NOT PORTED: importAll (Scope.java:884)
// - NOT PORTED: isFilled (Scope.java:898)
// - NOT PORTED: accepts (Scope.java:905)
// - NOT PORTED: doLookup (Scope.java:939)
// - NOT PORTED: doLookup (Scope.java:963)
// - NOT PORTED: isStaticallyImported (Scope.java:984)
// - NOT PORTED: isStaticallyImported (Scope.java:988)
// - NOT PORTED: importFrom (Scope.java:999)
// - NOT PORTED: doLookup (Scope.java:1013)
// - NOT PORTED: prependSubScope (Scope.java:1033)
// - NOT PORTED: appendSubScope (Scope.java:1042)
// - NOT PORTED: getMark (Scope.java:1061)
// - NOT PORTED: toString (Scope.java:1066)
// - NOT PORTED: isStaticallyImported (Scope.java:1110)
// End of Gap registration.

//! A scope represents an area of visibility in a Java program. The Scope class
//! is a container for symbols which provides efficient access to symbols given
//! their names. Scopes are implemented as hash tables with "open addressing"
//! and "double hashing". Scopes can be nested. Nested scopes can share their
//! hash tables.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs):
//! - a Scope is a value in the scope arena (ScopeId); the Entry storage is
//!   owned by the scope and entries refer to symbols by SymbolId;
//! - the Java abstract class hierarchy is represented by ScopeData variants
//!   (ScopeImpl / ErrorScope via the error_scope flag / CompoundScope /
//!   ImportScope / NamedImportScope / StarImportScope / SingleEntryScope);
//! - the upstream getIndex() reads the symbol name (and enterIfAbsent the
//!   symbol kind) through the Symbol object; the ported methods take the symbol
//!   arena slice for that read (the CompilerContext wrappers pass it, so the
//!   public API is CompilerContext-based);
//! - methods that need the receiver's arena ID (Entry.scope) take a
//!   self_id: ScopeId parameter (registered section 9);
//! - ITERATION ORDER IS PRESERVED: the hash table uses the same open
//!   addressing / double hashing algorithm as the upstream ScopeImpl (getIndex
//!   uses Name.get_index(), the Java Name.hashCode(), so the probing order is
//!   the upstream one) and the same elems / shadowed entry chains. No
//!   randomized hash container is used for the table; the visual (iteration)
//!   order is the upstream insertion-order-reversed elems chain.
//!   PORTING_RULES: getSymbols/getSymbolsByName return a Vec snapshot instead of
//!   a live iterator; this is equivalent for iterations without concurrent
//!   modification (registered section 9).
//!
//! NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic):
//! - dup/dupUnshared/leave: the upstream scopes share one Entry[] table object
//!   mutated through several scopes; that needs the ScopeArena table-sharing
//!   design (not implemented yet);
//! - cross-scope behavior (RECURSIVE getSymbols/getSymbolsByName, getOrigin,
//!   includes across outward scopes) needs the scope arena to resolve the next
//!   scope / entry scope;
//! - CompoundScope/ImportScope/NamedImportScope/StarImportScope/SingleEntryScope
//!   method bodies need Types/JCImport/CompletionFailure or cross-scope
//!   iteration; their structure and constructors ARE ported;
//! - ErrorScope.lookup override (the upstream returns a synthetic entry);
//! - ImportFilter/FilterImportScope/SymbolImporter.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::rc::Weak;

use crate::ids::{ScopeId, SymbolId};
use crate::code::kinds;
use crate::code::symbol::Symbol;
use crate::ids::TreeId;
use crate::util::arena::Arena;
use crate::util::list::List;
use crate::util::name::Name;

/// Java: public enum LookupKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupKind {
    RECURSIVE,
    NON_RECURSIVE,
}


/// Java: public interface ScopeListener
pub trait ScopeListener {
    fn symbol_added(&self, sym: Option<SymbolId>, scope: ScopeId);
    fn symbol_removed(&self, sym: Option<SymbolId>, scope: ScopeId);
}

/// Java: public static class ScopeListenerList
///
/// A list of scope listeners; listeners are stored in weak references, to avoid
/// memory leaks. When the listener list is scanned (upon notification), elements
/// corresponding to GC-ed listeners are removed so that the listener list size
/// is kept in check.
#[derive(Clone)]
pub struct ScopeListenerList {
    listeners: Vec<Weak<dyn ScopeListener>>,
}

impl ScopeListenerList {
    pub fn new() -> ScopeListenerList {
        ScopeListenerList {
            listeners: Vec::new(),
        }
    }

    /// Java: void add(ScopeListener sl) (the upstream prepends).
    pub fn add(&mut self, listener: Weak<dyn ScopeListener>) {
        self.listeners.insert(0, listener);
    }

    pub fn symbol_added(&mut self, sym: Option<SymbolId>, scope: ScopeId) {
        self.walk_references(sym, scope, false);
    }

    pub fn symbol_removed(&mut self, sym: Option<SymbolId>, scope: ScopeId) {
        self.walk_references(sym, scope, true);
    }

    fn walk_references(&mut self, sym: Option<SymbolId>, scope: ScopeId, is_remove: bool) {
        let mut new_listeners = Vec::new();
        for weak in std::mem::take(&mut self.listeners) {
            if let Some(listener) = weak.upgrade() {
                if is_remove {
                    listener.symbol_removed(sym, scope);
                } else {
                    listener.symbol_added(sym, scope);
                }
                new_listeners.push(weak);
            }
        }
        self.listeners = new_listeners;
    }
}

impl Default for ScopeListenerList {
    fn default() -> ScopeListenerList {
        ScopeListenerList::new()
    }
}

/// A scope entry index (the entry storage is owned by the scope).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(u32);

impl EntryId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// A hash table slot: the upstream ScopeImpl.table array of Entry objects,
/// where null is the empty slot and the sentinel marks a deleted entry.
#[derive(Clone, Debug)]
pub enum TableSlot {
    Empty,
    Deleted,
    Entry(EntryId),
}

/// Java: private static class Entry
#[derive(Clone, Debug)]
pub struct Entry {
    /// The referenced symbol (null iff this is the sentinel).
    pub sym: Option<SymbolId>,
    /// An entry with the same hash code, or sentinel.
    pub shadowed: Option<EntryId>,
    /// Next entry in same scope.
    pub next_sibling: Option<EntryId>,
    /// Prev entry in same scope.
    pub prev_sibling: Option<EntryId>,
    /// The entry's scope (null iff this is the sentinel).
    pub scope: Option<ScopeId>,
}

/// The scope's structure (one variant per upstream concrete Scope class; the
/// abstract Scope/WriteableScope behavior is expressed by the methods).
/// The ImportFilter predicate (Scope.ImportFilter.accepts(origin, sym)); the
/// compiler context is threaded explicitly because symbols live in the arena.
#[allow(clippy::type_complexity)]
pub type FilterImportFn =
    Rc<dyn Fn(&mut crate::compiler_context::CompilerContext, ScopeId, SymbolId) -> bool>;

#[derive(Clone)]
pub enum ScopeData {
    /// Java: private static class ScopeImpl (error_scope marks ErrorScope)
    ScopeImpl {
        /// The number of scopes that share this scope's hash table.
        shared: i32,
        /// Next enclosing scope (with whom this scope may share a hashtable).
        next: Option<ScopeId>,
        /// A hash table for the scope's entries.
        ///
        /// PORTING_RULES Porting rules adapter: upstream dup()/dupUnshared() share the
        /// Entry[] table object between scope instances; Rust represents the
        /// shared table/entry object graph with Rc<RefCell<...>> (borrow rule:
        /// never hold a borrow across a nested scope operation; the methods
        /// clone the Rc and re-borrow, see the module header).
        table: SharedTable,
        /// Java: int hashMask (always table.length - 1). Derived from the
        /// shared table length, so the field is not stored (registered mapping
        /// difference: internal representation only, no behavior change).
        /// A linear list that also contains all entries in reverse order of
        /// appearance (i.e later entries are pushed on top).
        elems: Option<EntryId>,
        /// The number of elements in this scope.
        /// This includes deleted elements, whose value is the sentinel.
        nelems: i32,
        remove_count: i32,
        /// The Entry storage (Java: the Entry objects referenced by the shared
        /// table; represented as one shared arena per scope chain).
        entries: SharedEntries,
        listeners: ScopeListenerList,
        /// Whether this is an ErrorScope (Java: ErrorScope extends ScopeImpl).
        error_scope: bool,
    },
    /// Java: public static class CompoundScope
    CompoundScope {
        sub_scopes: List<ScopeId>,
        mark: i32,
        listeners: ScopeListenerList,
    },
    /// Java: public static class ImportScope
    ImportScope {
        sub_scopes: List<ScopeId>,
        mark: i32,
        listeners: ScopeListenerList,
    },
    /// Java: public static class NamedImportScope
    NamedImportScope {
        sub_scopes: List<ScopeId>,
        mark: i32,
        listeners: ScopeListenerList,
        name2_scopes: HashMap<Name, Vec<ScopeId>>,
    },
    /// Java: public static class StarImportScope
    StarImportScope {
        sub_scopes: List<ScopeId>,
        mark: i32,
        listeners: ScopeListenerList,
    },
    /// Java: private static class NamedImportScope.SingleEntryScope
    SingleEntryScope {
        sym: Option<SymbolId>,
        content: List<SymbolId>,
        origin: Option<ScopeId>,
    },
    /// Java: private static class FilterImportScope extends Scope
    FilterImportScope {
        /// Java: private final Scope origin
        origin: ScopeId,
        /// Java: private final Name filterName (null for star imports)
        filter_name: Option<Name>,
        /// Java: private final ImportFilter filter (null for named imports)
        #[allow(clippy::type_complexity)]
        filter: Option<FilterImportFn>,
        /// Java: private final JCImport imp (arena tree id)
        imp: Option<TreeId>,
        /// Java: private final boolean staticImport
        static_import: bool,
        /// Java: private final BiConsumer<JCImport, CompletionFailure> cfHandler
        #[allow(clippy::type_complexity)]
    cf_handler: Option<Rc<dyn Fn(TreeId, &crate::code::symbol::CompletionFailure)>>,
    },
}

/// Java: public abstract class Scope (a scope value in the scope arena).
#[derive(Clone)]
pub struct Scope {
    /// The scope's owner.
    pub owner: Option<SymbolId>,
    pub data: ScopeData,
}

/// The initial table length (Java: ScopeImpl.INITIAL_SIZE).
pub const INITIAL_SIZE: usize = 0x10;

/// Shared ScopeImpl table (Java: the Entry[] shared by dup/leave).
pub(crate) type SharedTable = Rc<RefCell<Vec<TableSlot>>>;
/// Shared ScopeImpl entry storage (Java: the Entry object graph).
pub(crate) type SharedEntries = Rc<RefCell<Vec<Entry>>>;

impl Scope {
    /// Constructs a fresh writable scope (Java: new ScopeImpl(owner), which uses
    /// a fresh table of length INITIAL_SIZE).
    pub fn new_scope_impl(owner: Option<SymbolId>) -> Scope {
        Scope {
            owner,
            data: ScopeData::ScopeImpl {
                shared: 0,
                next: None,
                table: Rc::new(RefCell::new(vec![TableSlot::Empty; INITIAL_SIZE])),
                elems: None,
                nelems: 0,
                remove_count: 0,
                entries: Rc::new(RefCell::new(Vec::new())),
                listeners: ScopeListenerList::new(),
                error_scope: false,
            },
        }
    }

    /// Constructs an error scope (Java: new ErrorScope(errSymbol)).
    pub fn new_error_scope(owner: Option<SymbolId>) -> Scope {
        let mut scope = Scope::new_scope_impl(owner);
        if let ScopeData::ScopeImpl { error_scope, .. } = &mut scope.data {
            *error_scope = true;
        }
        scope
    }

    /// Java: public CompoundScope(Symbol owner)
    pub fn new_compound_scope(owner: Option<SymbolId>) -> Scope {
        Scope {
            owner,
            data: ScopeData::CompoundScope {
                sub_scopes: List::nil(),
                mark: 0,
                listeners: ScopeListenerList::new(),
            },
        }
    }

    /// Java: public ImportScope(Symbol owner)
    pub fn new_import_scope(owner: Option<SymbolId>) -> Scope {
        Scope {
            owner,
            data: ScopeData::ImportScope {
                sub_scopes: List::nil(),
                mark: 0,
                listeners: ScopeListenerList::new(),
            },
        }
    }

    /// Java: public NamedImportScope(Symbol owner)
    pub fn new_named_import_scope(owner: Option<SymbolId>) -> Scope {
        Scope {
            owner,
            data: ScopeData::NamedImportScope {
                sub_scopes: List::nil(),
                mark: 0,
                listeners: ScopeListenerList::new(),
                name2_scopes: HashMap::new(),
            },
        }
    }

    /// Java: public StarImportScope(Symbol owner)
    pub fn new_star_import_scope(owner: Option<SymbolId>) -> Scope {
        Scope {
            owner,
            data: ScopeData::StarImportScope {
                sub_scopes: List::nil(),
                mark: 0,
                listeners: ScopeListenerList::new(),
            },
        }
    }

    /// Java: NamedImportScope.SingleEntryScope(Symbol owner, Symbol sym, Scope origin)
    pub fn new_single_entry_scope(
        owner: Option<SymbolId>,
        sym: Option<SymbolId>,
        origin: Option<ScopeId>,
    ) -> Scope {
        let content = match sym {
            Some(symbol) => List::of(symbol),
            None => List::nil(),
        };
        Scope {
            owner,
            data: ScopeData::SingleEntryScope {
                sym,
                content,
                origin,
            },
        }
    }

    /// Java: public FilterImportScope(Types types, Scope origin, Name filterName,
    /// ImportFilter filter, JCImport imp, boolean staticImport,
    /// BiConsumer<JCImport, CompletionFailure> cfHandler)
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    pub fn new_filter_import_scope(
        owner: Option<SymbolId>,
        origin: ScopeId,
        filter_name: Option<Name>,
        filter: Option<FilterImportFn>,
        imp: Option<TreeId>,
        static_import: bool,
        cf_handler: Option<Rc<dyn Fn(TreeId, &crate::code::symbol::CompletionFailure)>>,
    ) -> Scope {
        Scope {
            owner,
            data: ScopeData::FilterImportScope {
                origin,
                filter_name,
                filter,
                imp,
                static_import,
                cf_handler,
            },
        }
    }

    /// Java: ScopeImpl.getIndex(Name name)
    ///
    /// Look for slot in the table. We use open addressing with double hashing.
    pub fn get_index(&self, name: &Name, symbols: &Arena<Symbol>) -> usize {
        let (table, entries) = self.table_and_entries();
        let mask = table.borrow().len() - 1;
        probe_index(name, &table.borrow(), mask, &entries.borrow(), symbols)
    }

    /// Clones the shared table/entry Rc handles (borrow rule: the caller must
    /// not keep a RefCell borrow across another method call).
    fn table_and_entries(&self) -> (SharedTable, SharedEntries) {
        let ScopeData::ScopeImpl { table, entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        (Rc::clone(table), Rc::clone(entries))
    }

    /// Java: public WriteableScope leave() (Scope.ScopeImpl.leave,
    /// Scope.java:382-400)
    ///
    /// Removes all entries of this scope from its shared table (if any) and
    /// returns the enclosing scope. The ported Scope owns its data behind
    /// Rc<RefCell<...>> shared handles; the next scope lives in the arena and is
    /// updated through the explicit scopes parameter (registered Porting rules adapter).
    pub fn leave(
        &self,
        self_id: ScopeId,
        scopes: &mut Arena<Scope>,
        symbols: &Arena<Symbol>,
    ) -> Option<ScopeId> {
        let (next, shared, table, elems, nelems, entries) = {
            let ScopeData::ScopeImpl {
                shared,
                next,
                table,
                elems,
                nelems,
                entries,
                ..
            } = &self.data
            else {
                panic!("ClassCastException: not a ScopeImpl")
            };
            (
                *next,
                *shared,
                Rc::clone(table),
                *elems,
                *nelems,
                Rc::clone(entries),
            )
        };
        let _ = self_id;
        // Java: Assert.check(shared == 0);
        assert_eq!(shared, 0, "AssertionError: shared == 0");
        let next_id = next.expect("NullPointerException: leave() on an outermost scope");
        let next_table = {
            let next_scope = scopes.get(next_id.index());
            match &next_scope.data {
                ScopeData::ScopeImpl { table, .. } => table.clone(),
                _ => panic!("ClassCastException: not a ScopeImpl"),
            }
        };
        // Java: if (table != next.table) return next;
        if !Rc::ptr_eq(&table, &next_table) {
            return Some(next_id);
        }
        {
            let mut table_ref = table.borrow_mut();
            let entries_ref = entries.borrow();
            let mask = table_ref.len() - 1;
            let mut current = elems;
            while let Some(entry_id) = current {
                let entry = &entries_ref[entry_id.index() as usize];
                let sym = entry.sym.expect("leave: entry without symbol");
                let name = symbol_name(symbols, sym);
                let hash = probe_index(&name, &table_ref, mask, &entries_ref, symbols);
                table_ref[hash] = match entry.shadowed {
                    Some(shadowed) => TableSlot::Entry(shadowed),
                    None => TableSlot::Empty,
                };
                current = entry.next_sibling;
            }
        }
        // Java: Assert.check(next.shared > 0); next.shared--; next.nelems = nelems;
        {
            let next_scope = scopes.get_mut(next_id.index());
            match &mut next_scope.data {
                ScopeData::ScopeImpl {
                    shared: next_shared,
                    nelems: next_nelems,
                    ..
                } => {
                    assert!(*next_shared > 0, "AssertionError: next.shared > 0");
                    *next_shared -= 1;
                    *next_nelems = nelems;
                }
                _ => panic!("ClassCastException: not a ScopeImpl"),
            }
        }
        Some(next_id)
    }

    /// Java: protected Entry lookup(Name name, Predicate<Symbol> sf)
    pub fn lookup(&self, name: &Name, symbols: &Arena<Symbol>) -> Option<EntryId> {
        let slot = self.get_index(name, symbols);
        let (table, entries) = self.table_and_entries();
        let table = table.borrow();
        let entries = entries.borrow();
        let mut current = match &table[slot] {
            TableSlot::Entry(entry_id) => Some(*entry_id),
            _ => None,
        };
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            let sym = entry.sym?;
            let entry_name = symbol_name(symbols, sym);
            if entry_name.to_java_string() == name.to_java_string() {
                return Some(entry_id);
            }
            current = entry.shadowed;
        }
        None
    }

    /// Java: public Symbol findFirst(Name name, Predicate<Symbol> sf)
    pub fn find_first(
        &self,
        name: &Name,
        filter: &dyn Fn(SymbolId) -> bool,
        symbols: &Arena<Symbol>,
    ) -> Option<SymbolId> {
        let (_, entries) = self.table_and_entries();
        let entries = entries.borrow();
        let mut current = self.lookup(name, symbols);
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            if let Some(sym) = entry.sym
                && filter(sym)
            {
                return Some(sym);
            }
            current = entry.shadowed;
        }
        None
    }

    /// Java: public void enter(Symbol sym)
    pub fn enter(&mut self, self_id: ScopeId, sym: SymbolId, symbols: &Arena<Symbol>) {
        let name = symbol_name(symbols, sym);
        {
            let ScopeData::ScopeImpl { shared, nelems, .. } = &self.data else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            assert!(*shared == 0, "shared scope cannot be modified");
            let table_len = {
                let (table, _) = self.table_and_entries();
                table.borrow().len()
            };
            if *nelems * 3 >= (table_len as i32 - 1) * 2 {
                self.dble(symbols);
            }
        }
        let slot = self.get_index(&name, symbols);
        let (table, entries) = self.table_and_entries();
        let (old_entry, entry_id) = {
            let mut table = table.borrow_mut();
            let old = std::mem::replace(&mut table[slot], TableSlot::Empty);
            let old_entry = match old {
                TableSlot::Entry(entry_id) => Some(entry_id),
                _ => None,
            };
            let mut entries = entries.borrow_mut();
            let entry_id = EntryId(entries.len() as u32);
            let elems = match &self.data {
                ScopeData::ScopeImpl { elems, .. } => *elems,
                _ => unreachable!(),
            };
            entries.push(Entry {
                sym: Some(sym),
                shadowed: old_entry,
                next_sibling: elems,
                prev_sibling: None,
                scope: Some(self_id),
            });
            if let Some(next) = elems {
                entries[next.index() as usize].prev_sibling = Some(entry_id);
            }
            table[slot] = TableSlot::Entry(entry_id);
            (old_entry, entry_id)
        };
        let ScopeData::ScopeImpl {
            elems,
            nelems,
            listeners,
            ..
        } = &mut self.data
        else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        if old_entry.is_none() {
            *nelems += 1;
        }
        *elems = Some(entry_id);
        listeners.symbol_added(Some(sym), self_id);
    }

    /// Java: public void enterIfAbsent(Symbol sym)
    pub fn enter_if_absent(
        &mut self,
        self_id: ScopeId,
        sym: SymbolId,
        symbols: &Arena<Symbol>,
    ) {
        let name = symbol_name(symbols, sym);
        let sym_kind = symbol_kind(symbols, sym);
        let (_, entries) = self.table_and_entries();
        let entries_ref = entries.borrow();
        let mut current = self.lookup(&name, symbols);
        while let Some(entry_id) = current {
            let entry = &entries_ref[entry_id.index() as usize];
            if entry.scope != Some(self_id) {
                break;
            }
            match entry.sym {
                Some(entry_sym) if symbol_kind(symbols, entry_sym) != sym_kind => {
                    current = entry.shadowed;
                }
                _ => break,
            }
        }
        let found_in_this = match current {
            Some(entry_id) => entries_ref[entry_id.index() as usize].scope == Some(self_id),
            None => false,
        };
        drop(entries_ref);
        if !found_in_this {
            self.enter(self_id, sym, symbols);
        }
    }

    /// Java: public void remove(Symbol sym)
    pub fn remove(&mut self, self_id: ScopeId, sym: SymbolId, symbols: &Arena<Symbol>) {
        let name = symbol_name(symbols, sym);
        let Some(entry_id) = self.lookup(&name, symbols) else {
            return;
        };
        let (table, entries) = self.table_and_entries();
        {
            let entries_ref = entries.borrow();
            let entry = &entries_ref[entry_id.index() as usize];
            if entry.sym != Some(sym) {
                return;
            }
        }
        let slot = self.get_index(&name, symbols);
        {
            let mut table = table.borrow_mut();
            let mut entries = entries.borrow_mut();
            // remove e from table and shadowed list;
            let mut handled = false;
            match &mut table[slot] {
                TableSlot::Entry(head) if *head == entry_id => {
                    let shadowed = entries[entry_id.index() as usize].shadowed;
                    match shadowed {
                        Some(id) => table[slot] = TableSlot::Entry(id),
                        None => table[slot] = TableSlot::Deleted,
                    }
                    handled = true;
                }
                TableSlot::Entry(head) => {
                    let mut te = *head;
                    loop {
                        let shadowed = entries[te.index() as usize].shadowed;
                        match shadowed {
                            Some(id) if id == entry_id => {
                                entries[te.index() as usize].shadowed =
                                    entries[entry_id.index() as usize].shadowed;
                                handled = true;
                                break;
                            }
                            Some(id) => te = id,
                            None => break,
                        }
                    }
                }
                _ => {},
            }
            let _ = handled;
            // remove e from elems and sibling list
            let prev = entries[entry_id.index() as usize].prev_sibling;
            let next = entries[entry_id.index() as usize].next_sibling;
            let ScopeData::ScopeImpl { elems, remove_count, listeners, .. } = &mut self.data else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            match prev {
                Some(prev_id) => {
                    entries[prev_id.index() as usize].next_sibling = next;
                }
                None => {
                    *elems = next;
                }
            }
            if let Some(next_id) = next {
                entries[next_id.index() as usize].prev_sibling = prev;
            }
            *remove_count += 1;
            listeners.symbol_removed(Some(sym), self_id);
        }
    }

    /// Java: public boolean includes(Symbol c) (ScopeImpl override)
    pub fn includes(
        &self,
        self_id: ScopeId,
        sym: SymbolId,
        symbols: &Arena<Symbol>,
    ) -> bool {
        let name = symbol_name(symbols, sym);
        let (_, entries) = self.table_and_entries();
        let entries = entries.borrow();
        let mut current = self.lookup(&name, symbols);
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            if entry.scope != Some(self_id) {
                break;
            }
            if entry.sym == Some(sym) {
                return true;
            }
            current = entry.shadowed;
        }
        false
    }

    /// Java: public boolean anyMatch(Predicate<Symbol> sf) (NON_RECURSIVE).
    pub fn any_match(&self, filter: &dyn Fn(SymbolId) -> bool) -> bool {
        let (_, entries) = self.table_and_entries();
        let entries = entries.borrow();
        let ScopeData::ScopeImpl { elems, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let mut current = *elems;
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            if let Some(sym) = entry.sym
                && filter(sym)
            {
                return true;
            }
            current = entry.next_sibling;
        }
        false
    }

    /// Java: public boolean isEmpty() (does not inspect outward scopes).
    pub fn is_empty(&self) -> bool {
        let (_, entries) = self.table_and_entries();
        let entries = entries.borrow();
        let ScopeData::ScopeImpl { elems, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let mut current = *elems;
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            if entry.sym.is_some() {
                return false;
            }
            current = entry.next_sibling;
        }
        true
    }

    /// Java: public WriteableScope dup(Symbol newOwner) — the new scope shares
    /// this scope's hash table and entry storage; this scope's shared count is
    /// incremented (ScopeImpl.dup).
    pub fn dup(&mut self, self_id: ScopeId, new_owner: Option<SymbolId>) -> Scope {
        let (table, entries, nelems) = {
            let (table, entries) = self.table_and_entries();
            let ScopeData::ScopeImpl { nelems, .. } = &self.data else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            (table, entries, *nelems)
        };
        let ScopeData::ScopeImpl { shared, .. } = &mut self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        *shared += 1;
        Scope {
            owner: new_owner,
            data: ScopeData::ScopeImpl {
                shared: 0,
                next: Some(self_id),
                table,
                elems: None,
                nelems,
                remove_count: 0,
                entries,
                listeners: ScopeListenerList::new(),
                error_scope: false,
            },
        }
    }

    /// Java: public WriteableScope dupUnshared(Symbol newOwner)
    pub fn dup_unshared(
        &self,
        self_id: ScopeId,
        new_owner: Option<SymbolId>,
        scopes: &Arena<Scope>,
    ) -> Scope {
        let (table, entries, nelems, shared) = {
            let (table, entries) = self.table_and_entries();
            let ScopeData::ScopeImpl { nelems, shared, .. } = &self.data else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            (table, entries, *nelems, *shared)
        };
        if shared > 0 {
            //The nested Scopes might have already added something to the table, so all items
            //that don't originate in this Scope or any of its outer Scopes need to be cleared:
            let mut accept_scopes = vec![self_id];
            let ScopeData::ScopeImpl { next, .. } = &self.data else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            let mut c = *next;
            while let Some(scope_id) = c {
                accept_scopes.push(scope_id);
                c = match &scopes.get(scope_id.index()).data {
                    ScopeData::ScopeImpl { next, .. } => *next,
                    _ => None,
                };
            }
            let (new_table, n) = {
                let old_table = table.borrow();
                let old_entries = entries.borrow();
                let mut new_table = vec![TableSlot::Empty; old_table.len()];
                let mut n = 0;
                for i in 0..old_table.len() {
                    let mut e = match &old_table[i] {
                        TableSlot::Entry(id) => Some(*id),
                        _ => None,
                    };
                    while let Some(id) = e {
                        let entry = &old_entries[id.index() as usize];
                        if entry.sym.is_some()
                            && entry.scope.is_some_and(|s| accept_scopes.contains(&s))
                        {
                            break;
                        }
                        e = entry.shadowed;
                    }
                    if let Some(id) = e {
                        n += 1;
                        new_table[i] = TableSlot::Entry(id);
                    }
                }
                (new_table, n)
            };
            Scope {
                owner: new_owner,
                data: ScopeData::ScopeImpl {
                    shared: 0,
                    next: Some(self_id),
                    table: Rc::new(RefCell::new(new_table)),
                    elems: None,
                    nelems: n,
                    remove_count: 0,
                    entries,
                    listeners: ScopeListenerList::new(),
                    error_scope: false,
                },
            }
        } else {
            let new_table = table.borrow().clone();
            Scope {
                owner: new_owner,
                data: ScopeData::ScopeImpl {
                    shared: 0,
                    next: Some(self_id),
                    table: Rc::new(RefCell::new(new_table)),
                    elems: None,
                    nelems,
                    remove_count: 0,
                    entries,
                    listeners: ScopeListenerList::new(),
                    error_scope: false,
                },
            }
        }
    }

    /// Double size of hash table (Java: ScopeImpl.dble()).
    fn dble(&mut self, symbols: &Arena<Symbol>) {
        let (table, entries) = self.table_and_entries();
        let mut table = table.borrow_mut();
        let new_len = table.len() * 2;
        let old_table = std::mem::replace(&mut *table, vec![TableSlot::Empty; new_len]);
        let old_len = old_table.len();
        let mask = table.len() - 1;
        let entries = entries.borrow();
        let mut n = 0i32;
        for index in (0..old_len).rev() {
            if let TableSlot::Entry(entry_id) = &old_table[index] {
                let entry_id = *entry_id;
                let sym = entries[entry_id.index() as usize]
                    .sym
                    .expect("entry without symbol");
                let name = symbol_name(symbols, sym);
                let slot = probe_index(&name, &table, mask, &entries, symbols);
                table[slot] = TableSlot::Entry(entry_id);
                n += 1;
            }
        }
        // We don't need to update nelems for shared inherited scopes,
        // since that gets handled by leave().
        let ScopeData::ScopeImpl { nelems, .. } = &mut self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        *nelems = n;
    }


    // ------------------------------------------------------------------
    // Scope query (Scope.java): full cross-scope queries for the import
    // scopes and the recursive ScopeImpl queries. The explicit-context adapter
    // passes the CompilerContext (ADR 0001); child scopes are cloned snapshots
    // so the immutable scope data and the mutable context do not borrow
    // simultaneously.
    // ------------------------------------------------------------------

    /// Java: public Iterable<Symbol> getSymbols(Predicate<Symbol> sf,
    /// LookupKind lookupKind)
    #[allow(clippy::too_many_arguments)]
    pub fn get_symbols_full(
        &self,
        self_id: ScopeId,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
        ctx: &mut crate::compiler_context::CompilerContext,
    ) -> Vec<SymbolId> {
        if !matches!(&self.data, ScopeData::FilterImportScope { .. }) {
            let ctx_view: &crate::compiler_context::CompilerContext = ctx;
            return self.get_symbols(self_id, lookup_kind, filter, ctx_view);
        }
        match &self.data {
            ScopeData::ScopeImpl { .. } => {
                // Java: ScopeImpl.getSymbols iterator (Scope.java:559-620):
                // walk the elems chain of the current scope, then (RECURSIVE)
                // continue with the next enclosing scope's elems chain.
                let mut result = Vec::new();
                let mut current_scope = self_id;
                loop {
                    let scope = if current_scope == self_id {
                        None
                    } else {
                        Some(ctx.scope(current_scope))
                    };
                    let (elems, next) = match scope {
                        None => match &self.data {
                            ScopeData::ScopeImpl { elems, next, .. } => (*elems, *next),
                            _ => unreachable!(),
                        },
                        Some(scope) => match &scope.data {
                            ScopeData::ScopeImpl { elems, next, .. } => (*elems, *next),
                            _ => break,
                        },
                    };
                    let entries = match scope {
                        None => self.table_and_entries().1,
                        Some(scope) => scope.table_and_entries().1,
                    };
                    let entries = entries.borrow();
                    let mut current = elems;
                    while let Some(entry_id) = current {
                        let entry = &entries[entry_id.index() as usize];
                        if let Some(sym) = entry.sym
                            && filter(sym)
                        {
                            result.push(sym);
                        }
                        current = entry.next_sibling;
                    }
                    if lookup_kind != LookupKind::RECURSIVE {
                        break;
                    }
                    match next {
                        Some(parent) => current_scope = parent,
                        None => break,
                    }
                }
                result
            }
            ScopeData::CompoundScope { sub_scopes, .. }
            | ScopeData::ImportScope { sub_scopes, .. }
            | ScopeData::NamedImportScope { sub_scopes, .. }
            | ScopeData::StarImportScope { sub_scopes, .. } => {
                let sub_scopes = sub_scopes.clone();
                let mut result = Vec::new();
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    result.extend(snapshot.get_symbols_full(sub_scope, lookup_kind, filter, types, syms, ctx));
                    current = current.tail();
                }
                result
            }
            ScopeData::SingleEntryScope { sym, content, .. } => match sym {
                Some(sym) if filter(*sym) => content.clone().to_vec(),
                _ => Vec::new(),
            },
            ScopeData::FilterImportScope {
                origin,
                filter_name,
                filter: import_filter,
                static_import,
                ..
            } => {
                let (origin, filter_name, static_import, import_filter) = (
                    *origin,
                    filter_name.clone(),
                    *static_import,
                    import_filter.clone(),
                );
                match filter_name {
                    Some(name) => self.get_symbols_by_name_full(
                        self_id,
                        &name,
                        lookup_kind,
                        filter,
                        types,
                        syms,
                        ctx,
                    ),
                    None => query_filter_import_scope_members(
                        origin,
                        static_import,
                        &import_filter,
                        None,
                        filter,
                        lookup_kind,
                        types,
                        syms,
                        ctx,
                    ),
                }
            }
        }
    }

    /// Java: public Iterable<Symbol> getSymbolsByName(Name name,
    /// Predicate<Symbol> sf, LookupKind lookupKind)
    #[allow(clippy::too_many_arguments)]
    pub fn get_symbols_by_name_full(
        &self,
        self_id: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
        ctx: &mut crate::compiler_context::CompilerContext,
    ) -> Vec<SymbolId> {
        if !matches!(&self.data, ScopeData::FilterImportScope { .. }) {
            let ctx_view: &crate::compiler_context::CompilerContext = ctx;
            let symbols = &ctx_view.symbols_arena();
            return self.get_symbols_by_name(self_id, name, lookup_kind, filter, symbols, ctx_view);
        }
        match &self.data {
            ScopeData::ScopeImpl { .. } => {
                let (_, entries) = self.table_and_entries();
                let entries = entries.borrow();
                let mut result = Vec::new();
                let start = {
                    let symbols = ctx.symbols_arena();
                    self.lookup(name, symbols)
                };
                let mut current = start;
                while let Some(entry_id) = current {
                    let entry = &entries[entry_id.index() as usize];
                    if lookup_kind == LookupKind::NON_RECURSIVE
                        && entry.scope != Some(self_id)
                    {
                        break;
                    }
                    match entry.sym {
                        Some(sym) => {
                            if filter(sym) {
                                result.push(sym);
                            }
                        }
                        None => break,
                    }
                    let mut next = entry.shadowed;
                    while let Some(next_id) = next {
                        let next_entry = &entries[next_id.index() as usize];
                        match next_entry.sym {
                            Some(next_sym) => {
                                if lookup_kind == LookupKind::NON_RECURSIVE
                                    && next_entry.scope != Some(self_id)
                                {
                                    next = None;
                                    break;
                                }
                                if filter(next_sym) {
                                    break;
                                }
                                next = next_entry.shadowed;
                            }
                            None => {
                                next = None;
                            }
                        }
                    }
                    current = next;
                }
                result
            }
            ScopeData::CompoundScope { sub_scopes, .. }
            | ScopeData::ImportScope { sub_scopes, .. } => {
                let sub_scopes = sub_scopes.clone();
                let mut result = Vec::new();
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    result.extend(
                        snapshot.get_symbols_by_name_full(sub_scope, name, lookup_kind, filter, types, syms, ctx),
                    );
                    current = current.tail();
                }
                result
            }
            ScopeData::NamedImportScope { name2_scopes, .. } => {
                let Some(scopes) = name2_scopes.get(name) else {
                    return Vec::new();
                };
                let scopes = scopes.clone();
                let mut result = Vec::new();
                for sub_scope in scopes {
                    let snapshot = ctx.scope(sub_scope).clone();
                    result.extend(
                        snapshot.get_symbols_by_name_full(sub_scope, name, lookup_kind, filter, types, syms, ctx),
                    );
                }
                result
            }
            ScopeData::StarImportScope { sub_scopes, .. } => {
                let sub_scopes = sub_scopes.clone();
                let mut result = Vec::new();
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    result.extend(
                        snapshot.get_symbols_by_name_full(sub_scope, name, lookup_kind, filter, types, syms, ctx),
                    );
                    current = current.tail();
                }
                result
            }
            ScopeData::SingleEntryScope { sym, content, .. } => match sym {
                Some(sym) => {
                    let matches = {
                        let symbol_name = ctx.sym(*sym).name.clone();
                        symbol_name == *name
                    };
                    if matches && filter(*sym) {
                        content.clone().to_vec()
                    } else {
                        Vec::new()
                    }
                }
                None => Vec::new(),
            },
            ScopeData::FilterImportScope {
                origin,
                filter_name,
                filter: import_filter,
                static_import,
                ..
            } => {
                if let Some(filter_name) = filter_name
                    && filter_name != name
                {
                    return Vec::new();
                }
                let (origin, static_import, import_filter) =
                    (*origin, *static_import, import_filter.clone());
                query_filter_import_scope_members(
                    origin,
                    static_import,
                    &import_filter,
                    Some(name),
                    filter,
                    lookup_kind,
                    types,
                    syms,
                    ctx,
                )
            }
        }
    }

    /// Java: Scope.getSymbols(Predicate<Symbol>, LookupKind) — the complete
    /// variant dispatch except FilterImportScope (whose SymbolImporter needs the
    /// Types/Symtab services; that path is get_symbols_full, used by Resolve).
    /// ScopeImpl implements the RECURSIVE next-chain semantics (Scope.java:559).
    pub fn get_symbols(
        &self,
        self_id: ScopeId,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        ctx: &crate::compiler_context::CompilerContext,
    ) -> Vec<SymbolId> {
        match &self.data {
            ScopeData::ScopeImpl { .. } => {
                let mut result = Vec::new();
                let mut current_scope = self_id;
                loop {
                    let (elems, next) = if current_scope == self_id {
                        match &self.data {
                            ScopeData::ScopeImpl { elems, next, .. } => (*elems, *next),
                            _ => unreachable!(),
                        }
                    } else {
                        match &ctx.scope(current_scope).data {
                            ScopeData::ScopeImpl { elems, next, .. } => (*elems, *next),
                            _ => break,
                        }
                    };
                    let entries = if current_scope == self_id {
                        self.table_and_entries().1
                    } else {
                        ctx.scope(current_scope).table_and_entries().1
                    };
                    let entries = entries.borrow();
                    let mut current = elems;
                    while let Some(entry_id) = current {
                        let entry = &entries[entry_id.index() as usize];
                        if let Some(sym) = entry.sym
                            && filter(sym)
                        {
                            result.push(sym);
                        }
                        current = entry.next_sibling;
                    }
                    if lookup_kind != LookupKind::RECURSIVE {
                        break;
                    }
                    match next {
                        Some(parent) => current_scope = parent,
                        None => break,
                    }
                }
                result
            }
            ScopeData::CompoundScope { sub_scopes, .. }
            | ScopeData::ImportScope { sub_scopes, .. }
            | ScopeData::NamedImportScope { sub_scopes, .. }
            | ScopeData::StarImportScope { sub_scopes, .. } => {
                let sub_scopes = sub_scopes.clone();
                let mut result = Vec::new();
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    result.extend(snapshot.get_symbols(sub_scope, lookup_kind, filter, ctx));
                    current = current.tail();
                }
                result
            }
            ScopeData::SingleEntryScope { sym, content, .. } => match sym {
                Some(sym) => {
                    if filter(*sym) {
                        content.clone().to_vec()
                    } else {
                        Vec::new()
                    }
                }
                None => Vec::new(),
            },
            ScopeData::FilterImportScope { .. } => {
                // TODO(Resolve): the FilterImportScope query needs the
                // SymbolImporter (Types.supertype/interfaces); use
                // get_symbols_full (the Resolve entry point) for that scope.
                unimplemented!(
                    "TODO(Resolve batch): FilterImportScope.getSymbols needs Types; use get_symbols_full"
                )
            }
        }
    }

    /// Java: Scope.getSymbolsByName(Name, Predicate<Symbol>, LookupKind) — the
    /// complete variant dispatch except FilterImportScope (see get_symbols).
    pub fn get_symbols_by_name(
        &self,
        self_id: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        symbols: &Arena<Symbol>,
        ctx: &crate::compiler_context::CompilerContext,
    ) -> Vec<SymbolId> {
        match &self.data {
            ScopeData::ScopeImpl { .. } => {
                let (_, entries) = self.table_and_entries();
                let entries = entries.borrow();
                let mut result = Vec::new();
                let mut current = self.lookup(name, symbols);
                while let Some(entry_id) = current {
                    let entry = &entries[entry_id.index() as usize];
                    let in_scope =
                        lookup_kind == LookupKind::RECURSIVE || entry.scope == Some(self_id);
                    if !in_scope {
                        break;
                    }
                    match entry.sym {
                        Some(sym) => {
                            if filter(sym) {
                                result.push(sym);
                            }
                        }
                        None => break,
                    }
                    let mut next = entry.shadowed;
                    while let Some(next_id) = next {
                        let next_entry = &entries[next_id.index() as usize];
                        if lookup_kind == LookupKind::NON_RECURSIVE
                            && next_entry.scope != Some(self_id)
                        {
                            next = None;
                            break;
                        }
                        match next_entry.sym {
                            Some(next_sym) => {
                                if filter(next_sym) {
                                    break;
                                }
                                next = next_entry.shadowed;
                            }
                            None => {
                                next = None;
                            }
                        }
                    }
                    current = next;
                }
                result
            }
            ScopeData::CompoundScope { sub_scopes, .. }
            | ScopeData::ImportScope { sub_scopes, .. }
            | ScopeData::NamedImportScope { sub_scopes, .. }
            | ScopeData::StarImportScope { sub_scopes, .. } => {
                let sub_scopes = sub_scopes.clone();
                let mut result = Vec::new();
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    result.extend(
                        snapshot.get_symbols_by_name(sub_scope, name, lookup_kind, filter, symbols, ctx),
                    );
                    current = current.tail();
                }
                result
            }
            ScopeData::SingleEntryScope { sym, content, .. } => match sym {
                Some(sym) => {
                    let matches = symbol_name(symbols, *sym) == *name;
                    if matches && filter(*sym) {
                        content.clone().to_vec()
                    } else {
                        Vec::new()
                    }
                }
                None => Vec::new(),
            },
            ScopeData::FilterImportScope { .. } => {
                // TODO(Resolve): see get_symbols; use get_symbols_by_name_full.
                unimplemented!(
                    "TODO(Resolve batch): FilterImportScope.getSymbolsByName needs Types; use get_symbols_by_name_full"
                )
            }
        }
    }

    /// Java: public boolean includes(Symbol c) — the Scope default
    /// implementation: getSymbolsByName(name, null, NON_RECURSIVE) contains c.
    pub fn includes_full(
        &self,
        self_id: ScopeId,
        sym: SymbolId,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
        ctx: &mut crate::compiler_context::CompilerContext,
    ) -> bool {
        let name = ctx.sym(sym).name.clone();
        let found = self.get_symbols_by_name_full(
            self_id,
            &name,
            LookupKind::NON_RECURSIVE,
            &|candidate| candidate == sym,
            types,
            syms,
            ctx,
        );
        !found.is_empty()
    }

    /// Java: public Scope getOrigin(Symbol byName)
    pub fn get_origin_full(
        &self,
        self_id: ScopeId,
        by_name: SymbolId,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
        ctx: &mut crate::compiler_context::CompilerContext,
    ) -> Option<ScopeId> {
        match &self.data {
            ScopeData::ScopeImpl { .. } => {
                let name = ctx.sym(by_name).name.clone();
                let (_, entries) = self.table_and_entries();
                let entries = entries.borrow();
                let start = {
                    let symbols = ctx.symbols_arena();
                    self.lookup(&name, symbols)
                };
                let mut current = start;
                while let Some(entry_id) = current {
                    let entry = &entries[entry_id.index() as usize];
                    if entry.scope != Some(self_id) {
                        return None;
                    }
                    if entry.sym == Some(by_name) {
                        return Some(self_id);
                    }
                    current = entry.shadowed;
                }
                None
            }
            ScopeData::SingleEntryScope { sym, origin, .. } => {
                if *sym == Some(by_name) {
                    *origin
                } else {
                    None
                }
            }
            ScopeData::FilterImportScope { origin, .. } => Some(*origin),
            ScopeData::CompoundScope { sub_scopes, .. }
            | ScopeData::ImportScope { sub_scopes, .. }
            | ScopeData::NamedImportScope { sub_scopes, .. }
            | ScopeData::StarImportScope { sub_scopes, .. } => {
                let sub_scopes = sub_scopes.clone();
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    if snapshot.includes_full(sub_scope, by_name, types, syms, ctx) {
                        return snapshot.get_origin_full(sub_scope, by_name, types, syms, ctx);
                    }
                    current = current.tail();
                }
                None
            }
        }
    }

    /// Java: public boolean isStaticallyImported(Symbol sym)
    pub fn is_statically_imported_full(
        &self,
        by_name: SymbolId,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
        ctx: &mut crate::compiler_context::CompilerContext,
    ) -> bool {
        match &self.data {
            ScopeData::FilterImportScope { static_import, .. } => *static_import,
            ScopeData::SingleEntryScope { .. } => false,
            ScopeData::ScopeImpl { .. } => false,
            ScopeData::NamedImportScope { .. }
            | ScopeData::CompoundScope { .. }
            | ScopeData::ImportScope { .. }
            | ScopeData::StarImportScope { .. } => {
                let sub_scopes = match &self.data {
                    ScopeData::CompoundScope { sub_scopes, .. }
                    | ScopeData::ImportScope { sub_scopes, .. }
                    | ScopeData::StarImportScope { sub_scopes, .. } => sub_scopes.clone(),
                    _ => unreachable!(),
                };
                let mut current = sub_scopes;
                while current.non_empty() {
                    let sub_scope = *current.head();
                    let snapshot = ctx.scope(sub_scope).clone();
                    if snapshot.includes_full(sub_scope, by_name, types, syms, ctx) {
                        return snapshot.is_statically_imported_full(by_name, types, syms, ctx);
                    }
                    current = current.tail();
                }
                false
            }
        }
    }
}
/// Reads the name of a symbol from the symbol arena (upstream: sym.name).
fn symbol_name(symbols: &Arena<Symbol>, sym: SymbolId) -> Name {
    symbols.get(sym.index()).name.clone()
}

/// Reads the kind of a symbol from the symbol arena (upstream: sym.kind).
fn symbol_kind(symbols: &Arena<Symbol>, sym: SymbolId) -> kinds::Kind {
    symbols.get(sym.index()).kind
}

/// Java: ScopeImpl.getIndex(Name name) (pure probe over the given table).
pub(crate) fn probe_index(
    name: &Name,
    table: &[TableSlot],
    hash_mask: usize,
    entries: &[Entry],
    symbols: &Arena<Symbol>,
) -> usize {
    let h = name.get_index();
    let mask = hash_mask as i32;
    let mut i = (h & mask) as usize;
    // The expression below is always odd, so it is guaranteed to be mutually
    // prime with table.length, a power of 2.
    let x = mask - ((h + (h >> 16)) << 1);
    let mut d: i32 = -1; // Index of a deleted item.
    loop {
        match &table[i] {
            TableSlot::Empty => return if d >= 0 { d as usize } else { i },
            TableSlot::Deleted => {
                // We have to keep searching even if we see a deleted item.
                // However, remember the index in case we fail to find the name.
                if d < 0 {
                    d = i as i32;
                }
            }
            TableSlot::Entry(entry_id) => {
                let entry = &entries[entry_id.index() as usize];
                let entry_sym = entry.sym.expect("sentinel entry stored in the table");
                let entry_name = symbol_name(symbols, entry_sym);
                if entry_name.get_index() == h
                    && entry_name.to_java_string() == name.to_java_string()
                {
                    return i;
                }
            }
        }
        i = ((i as i32 + x) & mask) as usize;
    }
}



/// Java: FilterImportScope's anonymous SymbolImporter.importFrom(TypeSymbol,
/// List<Iterable<Symbol>>) (Scope.java:999-1019). processed is the upstream
/// LinkedHashSet<Symbol> (identity semantics).
#[allow(clippy::too_many_arguments)]
fn symbol_importer_import_from(
    ctx: &mut crate::compiler_context::CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    inspect_super_types: bool,
    by_name: Option<&Name>,
    sf: &dyn Fn(SymbolId) -> bool,
    lookup_kind: LookupKind,
    processed: &mut Vec<SymbolId>,
    results: &mut Vec<Vec<SymbolId>>,
    tsym: Option<SymbolId>,
) {
    let Some(tsym) = tsym else {
        return;
    };
    if processed.contains(&tsym) {
        return;
    }
    processed.push(tsym);
    if inspect_super_types {
        let tsym_type = ctx.sym(tsym).ty.expect("type symbol type");
        let super_type = types.supertype(ctx, syms, tsym_type);
        let super_tsym = ctx.typ(super_type).tsym;
        symbol_importer_import_from(
            ctx,
            types,
            syms,
            inspect_super_types,
            by_name,
            sf,
            lookup_kind,
            processed,
            results,
            super_tsym,
        );
        let interfaces = types.interfaces(ctx, syms, tsym_type);
        let mut current = interfaces;
        while current.non_empty() {
            let interface = *current.head();
            let interface_tsym = ctx.typ(interface).tsym;
            symbol_importer_import_from(
                ctx,
                types,
                syms,
                inspect_super_types,
                by_name,
                sf,
                lookup_kind,
                processed,
                results,
                interface_tsym,
            );
            current = current.tail();
        }
    }
    let members = ctx.sym(tsym).members();
    let Some(members) = members else {
        results.insert(0, Vec::new());
        return;
    };
    let members_snapshot = ctx.scope(members).clone();
    let found = match by_name {
        Some(name) => {
            members_snapshot.get_symbols_by_name_full(members, name, lookup_kind, sf, types, syms, ctx)
        }
        None => members_snapshot.get_symbols_full(members, lookup_kind, sf, types, syms, ctx),
    };
    results.insert(0, found);
}

/// Java: FilterImportScope.getSymbols/getSymbolsByName + SymbolImporter +
/// filter.accepts(origin, s) (Scope.java:908-1019).
#[allow(clippy::too_many_arguments)]
fn query_filter_import_scope_members(
    origin: ScopeId,
    static_import: bool,
    import_filter: &Option<FilterImportFn>,
    by_name: Option<&Name>,
    sf: &dyn Fn(SymbolId) -> bool,
    lookup_kind: LookupKind,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    ctx: &mut crate::compiler_context::CompilerContext,
) -> Vec<SymbolId> {
    let origin_owner = ctx.scope(origin).owner.expect("import origin owner");
    let mut processed: Vec<SymbolId> = Vec::new();
    let mut results: Vec<Vec<SymbolId>> = Vec::new();
    symbol_importer_import_from(
        ctx,
        types,
        syms,
        static_import,
        by_name,
        sf,
        lookup_kind,
        &mut processed,
        &mut results,
        Some(origin_owner),
    );
    let mut output = Vec::new();
    for group in results {
        for sym in group {
            let accepted = match import_filter {
                Some(filter) => filter(ctx, origin, sym),
                None => panic!("AssertionError: FilterImportScope without filter"),
            };
            if accepted {
                output.push(sym);
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::kinds::Kind;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::util::arena::Arena;
    use crate::util::name::NameTableHandle;
    use std::rc::Rc;

    fn name_table() -> Rc<NameTableHandle> {
        NameTableHandle::unshared()
    }

    fn name(table: &Rc<NameTableHandle>, text: &str) -> Name {
        let units: Vec<u16> = text.encode_utf16().collect();
        table.from_chars(&units, 0, units.len() as i32)
    }

    fn symbol_arena(table: &Rc<NameTableHandle>, entries: &[(SymbolId, &str, Kind)]) -> Arena<Symbol> {
        let mut symbols = Arena::new();
        for (sym, text, kind) in entries {
            let index = symbols.push(Symbol::new(
                *kind,
                0,
                name(table, text),
                None,
                None,
                SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
            ));
            assert_eq!(index, sym.index());
        }
        symbols
    }

    fn scope_id(index: u32) -> ScopeId {
        ScopeId::new(index)
    }

    #[test]
    fn enter_lookup_and_iteration_order() {
        let table = name_table();
        let symbols = symbol_arena(
            &table,
            &[
                (SymbolId::new(0), "a", Kind::VAR),
                (SymbolId::new(1), "b", Kind::VAR),
                (SymbolId::new(2), "c", Kind::VAR),
            ],
        );
        let mut scope = Scope::new_scope_impl(None);
        let self_id = scope_id(0);
        scope.enter(self_id, SymbolId::new(0), &symbols);
        scope.enter(self_id, SymbolId::new(1), &symbols);
        scope.enter(self_id, SymbolId::new(2), &symbols);
        assert!(!scope.is_empty());
        assert_eq!(scope.get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new()).len(), 3);
        // The upstream elems chain keeps later entries on top.
        assert_eq!(
            scope.get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new()),
            vec![SymbolId::new(2), SymbolId::new(1), SymbolId::new(0)]
        );
        assert_eq!(
            scope.find_first(&name(&table, "b"), &|_| true, &symbols),
            Some(SymbolId::new(1))
        );
        assert_eq!(scope.lookup(&name(&table, "missing"), &symbols), None);
    }

    #[test]
    fn shadowed_entries_and_remove() {
        let table = name_table();
        let symbols = symbol_arena(
            &table,
            &[
                (SymbolId::new(0), "a", Kind::VAR),
                (SymbolId::new(1), "a", Kind::MTH),
            ],
        );
        let mut scope = Scope::new_scope_impl(None);
        let self_id = scope_id(0);
        scope.enter(self_id, SymbolId::new(0), &symbols);
        scope.enter(self_id, SymbolId::new(1), &symbols);
        assert_eq!(
            scope.get_symbols_by_name(
                self_id,
                &name(&table, "a"),
                LookupKind::NON_RECURSIVE,
                &|_| true,
                &symbols,
                &crate::compiler_context::CompilerContext::new()
            ),
            vec![SymbolId::new(1), SymbolId::new(0)]
        );
        assert!(scope.includes(self_id, SymbolId::new(0), &symbols));
        // remove the top entry: the shadowed one becomes visible.
        scope.remove(self_id, SymbolId::new(1), &symbols);
        assert!(!scope.includes(self_id, SymbolId::new(1), &symbols));
        assert!(scope.includes(self_id, SymbolId::new(0), &symbols));
        assert_eq!(
            scope.get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new()),
            vec![SymbolId::new(0)]
        );
    }

    #[test]
    fn enter_if_absent_respects_symbol_kind() {
        let table = name_table();
        let symbols = symbol_arena(
            &table,
            &[
                (SymbolId::new(0), "a", Kind::VAR),
                (SymbolId::new(1), "a", Kind::VAR),
                (SymbolId::new(2), "a", Kind::MTH),
            ],
        );
        let mut scope = Scope::new_scope_impl(None);
        let self_id = scope_id(0);
        scope.enter(self_id, SymbolId::new(0), &symbols);
        // same name and same kind: not entered.
        scope.enter_if_absent(self_id, SymbolId::new(1), &symbols);
        assert_eq!(scope.get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new()).len(), 1);
        // same name, different kind: entered.
        scope.enter_if_absent(self_id, SymbolId::new(2), &symbols);
        assert_eq!(scope.get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new()).len(), 2);
    }

    #[test]
    fn table_doubles_and_keeps_all_entries() {
        let table = name_table();
        let mut entries = Vec::new();
        let mut symbols = Arena::new();
        for index in 0..40u32 {
            let text = format!("name{index}");
            let units: Vec<u16> = text.encode_utf16().collect();
            let sym = symbols.push(Symbol::new(
                Kind::VAR,
                0,
                table.from_chars(&units, 0, units.len() as i32),
                None,
                None,
                SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
            ));
            entries.push(SymbolId::new(sym));
        }
        let mut scope = Scope::new_scope_impl(None);
        let self_id = scope_id(0);
        for sym in &entries {
            scope.enter(self_id, *sym, &symbols);
        }
        let all = scope.get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new());
        assert_eq!(all.len(), 40);
        // The elems chain order is the reverse of the insertion order.
        assert_eq!(all.first(), Some(&SymbolId::new(39)));
        assert_eq!(all.last(), Some(&SymbolId::new(0)));
        // All names are still found after the rehashing.
        for sym in &entries {
            let symbol_name = symbol_name(&symbols, *sym);
            let entry = scope.lookup(&symbol_name, &symbols).expect("entry");
            let ScopeData::ScopeImpl { entries, .. } = &scope.data else {
                panic!("expected a ScopeImpl");
            };
            let entries = entries.borrow();
            assert_eq!(entries[entry.index() as usize].sym, Some(*sym));
        }
    }

    #[test]
    fn error_scope_flag_is_recorded() {
        let scope = Scope::new_error_scope(None);
        let self_id = scope_id(0);
        match scope.data {
            ScopeData::ScopeImpl { error_scope, .. } => assert!(error_scope),
            _ => panic!("expected an error scope"),
        }
        assert_eq!(
            scope
                .get_symbols(self_id, LookupKind::NON_RECURSIVE, &|_| true, &crate::compiler_context::CompilerContext::new())
                .len(),
            0
        );
    }

    #[test]
    fn single_entry_scope_structure() {
        let origin = scope_id(2);
        let scope = Scope::new_single_entry_scope(
            Some(SymbolId::new(9)),
            Some(SymbolId::new(1)),
            Some(origin),
        );
        match scope.data {
            ScopeData::SingleEntryScope { sym, content, .. } => {
                assert_eq!(sym, Some(SymbolId::new(1)));
                assert_eq!(content.length(), 1);
            }
            _ => panic!("expected a single entry scope"),
        }
    }

    /// Java: ScopeImpl.getSymbols(RECURSIVE) walks the next-scope chain
    /// (Scope.java:559-620 update(): currScope = currScope.next while
    /// currEntry == null). This is the scope chain Attr creates with
    /// scope.dup()/scope.leave() for nested blocks/method bodies.
    #[test]
    fn recursive_iteration_follows_the_next_scope_chain() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let table = name_table();
        let a_name = name(&table, "a");
        let b_name = name(&table, "b");
        let outer = ctx.scope_create(None);
        let a = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            a_name.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        ctx.scope_enter(outer, a);
        // dup() creates the inner scope: it shares the table and points at the
        // outer scope through next (ScopeImpl.dup).
        let inner = ctx.scope_dup(outer, None);
        let b = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            b_name.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        ctx.scope_enter(inner, b);
        // NON_RECURSIVE returns only the inner scope's own entries.
        assert_eq!(
            ctx.scope_get_symbols(inner, LookupKind::NON_RECURSIVE, &|_| true),
            vec![b]
        );
        // RECURSIVE continues with the outer scope's elems chain.
        let mut recursive = ctx.scope_get_symbols(inner, LookupKind::RECURSIVE, &|_| true);
        recursive.sort_by_key(|sym| sym.index());
        assert_eq!(recursive, vec![a, b]);
        // The default name lookup (Scope.java:92-114 is RECURSIVE) sees the
        // outer scope's entries through the shared table.
        let by_name =
            ctx.scope_get_symbols_by_name(inner, &a_name, LookupKind::RECURSIVE, &|_| true);
        assert_eq!(by_name, vec![a]);
    }

    /// The Attr method-body/block scope pattern: the inner (dup-ed) scope sees
    /// the enclosing scope names through the default RECURSIVE lookup, while
    /// NON_RECURSIVE does not (Scope.java:573-620).
    #[test]
    fn default_recursive_lookup_sees_enclosing_scope_names() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let table = name_table();
        let walk = name(&table, "walk");
        let outer = ctx.scope_create(None);
        let walk_sym = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            walk.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        ctx.scope_enter(outer, walk_sym);
        let inner = ctx.scope_dup(outer, None);
        // The default Scope.getSymbolsByName(name) is RECURSIVE.
        let recursive = ctx.scope_get_symbols_by_name(
            inner,
            &walk,
            LookupKind::RECURSIVE,
            &|_| true,
        );
        assert_eq!(recursive, vec![walk_sym]);
        let non_recursive = ctx.scope_get_symbols_by_name(
            inner,
            &walk,
            LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        assert!(non_recursive.is_empty());
        //... and the RECURSIVE getSymbols visits the enclosing elems chain.
        let all = ctx.scope_get_symbols(inner, LookupKind::RECURSIVE, &|_| true);
        assert_eq!(all, vec![walk_sym]);
    }

    /// CompoundScope.getSymbols/getSymbolsByName compose the sub scopes in
    /// order (Scope.java:1024-1103).
    #[test]
    fn compound_scope_composes_sub_scope_queries() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let table = name_table();
        let alpha = name(&table, "alpha");
        let beta = name(&table, "beta");
        let a = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            alpha.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let b = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            beta.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let first = ctx.scope_create(None);
        ctx.scope_enter(first, a);
        let second = ctx.scope_create(None);
        ctx.scope_enter(second, b);
        let compound = ctx.alloc_scope(crate::code::scope::Scope::new_compound_scope(None));
        ctx.scope_append_sub_scope(compound, first);
        ctx.scope_append_sub_scope(compound, second);
        let all = ctx.scope_get_symbols(compound, LookupKind::NON_RECURSIVE, &|_| true);
        assert_eq!(all, vec![a, b]);
        let by_alpha =
            ctx.scope_get_symbols_by_name(compound, &alpha, LookupKind::NON_RECURSIVE, &|_| true);
        assert_eq!(by_alpha, vec![a]);
        let by_beta =
            ctx.scope_get_symbols_by_name(compound, &beta, LookupKind::NON_RECURSIVE, &|_| true);
        assert_eq!(by_beta, vec![b]);
    }

    /// NamedImportScope.getSymbolsByName uses the name2Scopes index
    /// (Scope.java:820-826).
    #[test]
    fn named_import_scope_uses_the_name_index() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let table = name_table();
        let target = name(&table, "Target");
        let origin_scope = ctx.scope_create(None);
        let named = ctx.alloc_scope(crate::code::scope::Scope::new_named_import_scope(None));
        let origin_sym = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            target.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let single = ctx.alloc_scope(crate::code::scope::Scope::new_single_entry_scope(
            None,
            Some(origin_sym),
            Some(origin_scope),
        ));
        ctx.scope_import_type(named, single, origin_scope, origin_sym);
        let found =
            ctx.scope_get_symbols_by_name(named, &target, LookupKind::NON_RECURSIVE, &|_| true);
        assert_eq!(found, vec![origin_sym]);
        let missing =
            ctx.scope_get_symbols_by_name(named, &name(&table, "other"), LookupKind::NON_RECURSIVE, &|_| true);
        assert!(missing.is_empty());
    }

    #[test]
    fn dup_shares_the_table_and_leave_undoes_entries() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let table = name_table();
        let walk = name(&table, "walk");
        let run = name(&table, "run");
        let first = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            walk.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let second = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            run.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let outer = ctx.scope_create(None);
        ctx.scope_enter(outer, first);
        let inner = ctx.scope_dup(outer, None);
        // Java: the dup-ed scope shares the table, so an entry entered in the
        // inner scope is visible from the outer scope until leave() removes it.
        ctx.scope_enter(inner, second);
        assert!(ctx.scope_lookup(outer, &run).is_some());
        let back = ctx.scope_leave(inner);
        assert_eq!(back, outer);
        assert!(ctx.scope_lookup(outer, &run).is_none());
        assert!(ctx.scope_lookup(outer, &walk).is_some());
    }

    #[test]
    fn dup_unshared_isolates_new_entries() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let table = name_table();
        let walk = name(&table, "walk");
        let run = name(&table, "run");
        let first = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            walk.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let second = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            run.clone(),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None , data: None},
        ));
        let outer = ctx.scope_create(None);
        ctx.scope_enter(outer, first);
        let inner = ctx.scope_dup_unshared(outer, None);
        // dupUnshared copies the table contents but not the shared structure.
        assert!(ctx.scope_lookup(inner, &walk).is_some());
        ctx.scope_enter(inner, second);
        assert!(ctx.scope_lookup(inner, &run).is_some());
        assert!(ctx.scope_lookup(outer, &run).is_none());
    }
    #[test]
    fn leave_removes_entries_and_updates_next_scope() {
        let table_handle = name_table();
        let sym_id = SymbolId::new(0);
        let owner_id = SymbolId::new(1);
        let mut symbols = symbol_arena(
            &table_handle,
            &[
                (sym_id, "x", Kind::VAR),
                (owner_id, "Sample", Kind::TYP),
            ],
        );
        let mut scopes: Arena<Scope> = Arena::new();
        let outer_id = ScopeId::new(scopes.push(Scope::new_scope_impl(Some(owner_id))));
        let mut outer = scopes.get(outer_id.index()).clone();
        let outer_dup = outer.dup(outer_id, Some(owner_id));
        *scopes.get_mut(outer_id.index()) = outer;
        let inner_id = ScopeId::new(scopes.push(outer_dup));
        {
            let inner = scopes.get_mut(inner_id.index());
            inner.enter(inner_id, sym_id, &symbols);
        }
        let x_name = name(&table_handle, "x");
        {
            let inner = scopes.get(inner_id.index());
            assert!(inner.lookup(&x_name, &symbols).is_some());
        }
        let left = {
            let inner = scopes.get(inner_id.index()).clone();
            inner.leave(inner_id, &mut scopes, &symbols)
        };
        assert_eq!(left, Some(outer_id));
        let outer_after = scopes.get(outer_id.index());
        match &outer_after.data {
            ScopeData::ScopeImpl { shared, .. } => assert_eq!(*shared, 0),
            _ => panic!("expected ScopeImpl"),
        }
        assert!(outer_after.lookup(&x_name, &symbols).is_none());
        let _ = &mut symbols;
    }

}
