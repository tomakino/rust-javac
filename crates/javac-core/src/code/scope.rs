// Origin: com/sun/tools/javac/code/Scope.java @ 982d53c (jdk-17.0.19-ga)

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

use std::collections::HashMap;
use std::rc::Weak;

use crate::ids::{ScopeId, SymbolId};
use crate::code::kinds;
use crate::code::symbol::Symbol;
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
pub enum ScopeData {
    /// Java: private static class ScopeImpl (error_scope marks ErrorScope)
    ScopeImpl {
        /// The number of scopes that share this scope's hash table.
        shared: i32,
        /// Next enclosing scope (with whom this scope may share a hashtable).
        next: Option<ScopeId>,
        /// A hash table for the scope's entries.
        table: Vec<TableSlot>,
        /// Mask for hash codes, always equal to (table.length - 1).
        hash_mask: usize,
        /// A linear list that also contains all entries in reverse order of
        /// appearance (i.e later entries are pushed on top).
        elems: Option<EntryId>,
        /// The number of elements in this scope.
        /// This includes deleted elements, whose value is the sentinel.
        nelems: i32,
        remove_count: i32,
        /// The Entry storage owned by this scope.
        entries: Vec<Entry>,
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
}

/// Java: public abstract class Scope (a scope value in the scope arena).
pub struct Scope {
    /// The scope's owner.
    pub owner: Option<SymbolId>,
    pub data: ScopeData,
}

/// The initial table length (Java: ScopeImpl.INITIAL_SIZE).
pub const INITIAL_SIZE: usize = 0x10;

impl Scope {
    /// Constructs a fresh writable scope (Java: new ScopeImpl(owner), which uses
    /// a fresh table of length INITIAL_SIZE).
    pub fn new_scope_impl(owner: Option<SymbolId>) -> Scope {
        Scope {
            owner,
            data: ScopeData::ScopeImpl {
                shared: 0,
                next: None,
                table: vec![TableSlot::Empty; INITIAL_SIZE],
                hash_mask: INITIAL_SIZE - 1,
                elems: None,
                nelems: 0,
                remove_count: 0,
                entries: Vec::new(),
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

    /// Java: ScopeImpl.getIndex(Name name)
    ///
    /// Look for slot in the table. We use open addressing with double hashing.
    pub fn get_index(&self, name: &Name, symbols: &Arena<Symbol>) -> usize {
        let ScopeData::ScopeImpl {
            table,
            hash_mask,
            entries,
            ..
        } = &self.data
        else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        probe_index(name, table, *hash_mask, entries, symbols)
    }

    /// Java: protected Entry lookup(Name name, Predicate<Symbol> sf)
    pub fn lookup(&self, name: &Name, symbols: &Arena<Symbol>) -> Option<EntryId> {
        let slot = self.get_index(name, symbols);
        let ScopeData::ScopeImpl { table, entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
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
        let ScopeData::ScopeImpl { entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
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
            let ScopeData::ScopeImpl {
                shared,
                hash_mask,
                nelems,
                ..
            } = &self.data
            else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            assert!(*shared == 0, "shared scope cannot be modified");
            if *nelems * 3 >= *hash_mask as i32 * 2 {
                self.dble(symbols);
            }
        }
        let slot = self.get_index(&name, symbols);
        let ScopeData::ScopeImpl {
            table,
            elems,
            nelems,
            entries,
            listeners,
            ..
        } = &mut self.data
        else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let old = std::mem::replace(&mut table[slot], TableSlot::Empty);
        let old_entry = match old {
            TableSlot::Entry(entry_id) => Some(entry_id),
            _ => None,
        };
        if old_entry.is_none() {
            *nelems += 1;
        }
        let entry_id = EntryId(entries.len() as u32);
        entries.push(Entry {
            sym: Some(sym),
            shadowed: old_entry,
            next_sibling: *elems,
            prev_sibling: None,
            scope: Some(self_id),
        });
        if let Some(next) = *elems {
            entries[next.index() as usize].prev_sibling = Some(entry_id);
        }
        table[slot] = TableSlot::Entry(entry_id);
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
        let ScopeData::ScopeImpl { entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let mut current = self.lookup(&name, symbols);
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
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
            Some(entry_id) => entries[entry_id.index() as usize].scope == Some(self_id),
            None => false,
        };
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
        {
            let ScopeData::ScopeImpl { entries, .. } = &self.data else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            let entry = &entries[entry_id.index() as usize];
            if entry.sym != Some(sym) {
                return;
            }
        }
        let slot = self.get_index(&name, symbols);
        let ScopeData::ScopeImpl {
            table,
            elems,
            entries,
            remove_count,
            listeners,
            ..
        } = &mut self.data
        else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        // remove e from table and shadowed list;
        match &mut table[slot] {
            TableSlot::Entry(head) if *head == entry_id => {
                let shadowed = entries[entry_id.index() as usize].shadowed;
                match shadowed {
                    Some(id) => table[slot] = TableSlot::Entry(id),
                    None => table[slot] = TableSlot::Deleted,
                }
            }
            TableSlot::Entry(head) => {
                let mut te = *head;
                loop {
                    let shadowed = entries[te.index() as usize].shadowed;
                    match shadowed {
                        Some(id) if id == entry_id => {
                            entries[te.index() as usize].shadowed =
                                entries[entry_id.index() as usize].shadowed;
                            break;
                        }
                        Some(id) => te = id,
                        None => break,
                    }
                }
            }
            _ => {}
        }
        // remove e from elems and sibling list
        let prev = entries[entry_id.index() as usize].prev_sibling;
        let next = entries[entry_id.index() as usize].next_sibling;
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
        //notify listeners
        listeners.symbol_removed(Some(sym), self_id);
    }

    /// Java: public boolean includes(Symbol c) (ScopeImpl override)
    pub fn includes(
        &self,
        self_id: ScopeId,
        sym: SymbolId,
        symbols: &Arena<Symbol>,
    ) -> bool {
        let name = symbol_name(symbols, sym);
        let ScopeData::ScopeImpl { entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
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
        let ScopeData::ScopeImpl { elems, entries, .. } = &self.data else {
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
        let ScopeData::ScopeImpl { elems, entries, .. } = &self.data else {
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

    /// Java: ScopeImpl.getSymbols(Predicate<Symbol>, LookupKind)
    ///
    /// PORTING_RULES: returns a Vec snapshot (see the module doc); the order is
    /// the upstream elems chain order (most recently entered first).
    pub fn get_symbols(
        &self,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        if lookup_kind == LookupKind::RECURSIVE {
            panic!("unimplemented: recursive scope iteration needs the scope arena");
        }
        let ScopeData::ScopeImpl { elems, entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let mut result = Vec::new();
        let mut current = *elems;
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            if let Some(sym) = entry.sym
                && filter(sym)
            {
                result.push(sym);
            }
            current = entry.next_sibling;
        }
        result
    }

    /// Java: ScopeImpl.getSymbolsByName(Name, Predicate<Symbol>, LookupKind)
    ///
    /// PORTING_RULES: returns a Vec snapshot (see the module doc); the order is
    /// the upstream shadowed chain order (most recently entered first).
    pub fn get_symbols_by_name(
        &self,
        self_id: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        symbols: &Arena<Symbol>,
    ) -> Vec<SymbolId> {
        if lookup_kind == LookupKind::RECURSIVE {
            panic!("unimplemented: recursive scope iteration needs the scope arena");
        }
        let ScopeData::ScopeImpl { entries, .. } = &self.data else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let mut result = Vec::new();
        let mut current = self.lookup(name, symbols);
        while let Some(entry_id) = current {
            let entry = &entries[entry_id.index() as usize];
            if entry.scope != Some(self_id) {
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
            // Java: Entry.next(Predicate<Symbol> sf)
            let mut next = entry.shadowed;
            while let Some(next_id) = next {
                let next_entry = &entries[next_id.index() as usize];
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

    /// Double size of hash table (Java: ScopeImpl.dble()).
    fn dble(&mut self, symbols: &Arena<Symbol>) {
        let ScopeData::ScopeImpl {
            table,
            hash_mask,
            entries,
            nelems,
            ..
        } = &mut self.data
        else {
            panic!("ClassCastException: not a ScopeImpl");
        };
        let old_table = std::mem::take(table);
        let old_len = old_table.len();
        table.resize(old_len * 2, TableSlot::Empty);
        *hash_mask = table.len() - 1;
        let mask = *hash_mask;
        let mut n = 0i32;
        for index in (0..old_len).rev() {
            if let TableSlot::Entry(entry_id) = &old_table[index] {
                let entry_id = *entry_id;
                let sym = entries[entry_id.index() as usize]
                    .sym
                    .expect("entry without symbol");
                let name = symbol_name(symbols, sym);
                let slot = probe_index(&name, table, mask, entries, symbols);
                table[slot] = TableSlot::Entry(entry_id);
                n += 1;
            }
        }
        // We don't need to update nelems for shared inherited scopes,
        // since that gets handled by leave().
        *nelems = n;
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
fn probe_index(
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
                SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None },
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
        assert_eq!(scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true).len(), 3);
        // The upstream elems chain keeps later entries on top.
        assert_eq!(
            scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true),
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
                &symbols
            ),
            vec![SymbolId::new(1), SymbolId::new(0)]
        );
        assert!(scope.includes(self_id, SymbolId::new(0), &symbols));
        // remove the top entry: the shadowed one becomes visible.
        scope.remove(self_id, SymbolId::new(1), &symbols);
        assert!(!scope.includes(self_id, SymbolId::new(1), &symbols));
        assert!(scope.includes(self_id, SymbolId::new(0), &symbols));
        assert_eq!(
            scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true),
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
        assert_eq!(scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true).len(), 1);
        // same name, different kind: entered.
        scope.enter_if_absent(self_id, SymbolId::new(2), &symbols);
        assert_eq!(scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true).len(), 2);
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
                SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None },
            ));
            entries.push(SymbolId::new(sym));
        }
        let mut scope = Scope::new_scope_impl(None);
        let self_id = scope_id(0);
        for sym in &entries {
            scope.enter(self_id, *sym, &symbols);
        }
        let all = scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true);
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
            assert_eq!(entries[entry.index() as usize].sym, Some(*sym));
        }
    }

    #[test]
    fn error_scope_flag_is_recorded() {
        let scope = Scope::new_error_scope(None);
        match scope.data {
            ScopeData::ScopeImpl { error_scope, .. } => assert!(error_scope),
            _ => panic!("expected an error scope"),
        }
        assert_eq!(scope.get_symbols(LookupKind::NON_RECURSIVE, &|_| true).len(), 0);
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

    #[test]
    #[should_panic(expected = "recursive scope iteration")]
    fn recursive_iteration_is_not_ported() {
        let table = name_table();
        let mut symbols = Arena::new();
        symbols.push(Symbol::new(
            Kind::VAR,
            0,
            name(&table, "a"),
            None,
            None,
            SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None },
        ));
        let scope = Scope::new_scope_impl(None);
        let _ = scope.get_symbols(LookupKind::RECURSIVE, &|_| true);
    }
}
