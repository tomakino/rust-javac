// Origin: com/sun/tools/javac/code/TypeMetadata.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES:
// - TypeMetadata is an immutable EnumMap<Entry.Kind, Entry>; the port uses a
// BTreeMap keyed by EntryKind (unique key per kind, deterministic iteration).
// - The Java EMPTY singleton identity comparisons map to the empty-contents
// check (EMPTY is the unique empty map and TypeMetadata is immutable).
// - Attribute.Compound/TypeCompound references map to AttributeId (arena, adapter).
// - Annotations.TO_BE_SET is modelled as annos: None (the Java code uses the
// empty-list identity to detect the to-be-set state); combine asserts that
// state exactly like the upstream Assert.

use std::collections::BTreeMap;

use crate::ids::AttributeId;
use crate::util::list::List;

/// Java: TypeMetadata.Entry.Kind (an enum with the single ANNOTATIONS value).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    ANNOTATIONS,
}

/// Java: interface TypeMetadata.Entry.
#[derive(Clone, Debug)]
pub enum Entry {
    /// Java: TypeMetadata.Annotations
    Annotations(Annotations),
}

impl Entry {
    /// Java: public Kind kind()
    pub fn kind(&self) -> EntryKind {
        match self {
            Entry::Annotations(_) => EntryKind::ANNOTATIONS,
        }
    }

    /// Java: public Entry combine(Entry other)
    pub fn combine(self, other: Entry) -> Entry {
        match (self, other) {
            (Entry::Annotations(mut left), Entry::Annotations(right)) => {
                left.combine(&right);
                Entry::Annotations(left)
            }
        }
    }
}

/// Java: public static class TypeMetadata.Annotations implements Entry.
#[derive(Clone, Debug)]
pub struct Annotations {
    /// Java: private List<Attribute.TypeCompound> annos (TO_BE_SET -> None).
    annos: Option<List<AttributeId>>,
}

impl Annotations {
    /// Java: public Annotations(List<Attribute.TypeCompound> annos)
    pub fn new(annos: List<AttributeId>) -> Annotations {
        Annotations { annos: Some(annos) }
    }

    /// Java: public static final List<Attribute.TypeCompound> TO_BE_SET
    pub fn to_be_set() -> Annotations {
        Annotations { annos: None }
    }

    /// Java: public List<Attribute.TypeCompound> getAnnotations()
    pub fn get_annotations(&self) -> List<AttributeId> {
        self.annos.clone().unwrap_or_else(List::nil)
    }

    /// Java: public Annotations combine(Entry other)
    ///
    /// Java: Assert.check(annos == TO_BE_SET); annos = ((Annotations)other).annos;
    pub fn combine(&mut self, other: &Annotations) {
        assert!(
            self.annos.is_none(),
            "AssertionError: TypeMetadata.Annotations.combine on a set entry"
        );
        self.annos = other.annos.clone();
    }

    /// Java: public Kind kind() { return Kind.ANNOTATIONS; }
    pub fn kind(&self) -> EntryKind {
        EntryKind::ANNOTATIONS
    }
}

/// Java: public class TypeMetadata
#[derive(Clone, Debug, Default)]
pub struct TypeMetadata {
    /// Java: private final EnumMap<Entry.Kind, Entry> contents
    contents: BTreeMap<EntryKind, Entry>,
}

impl TypeMetadata {
    /// Java: public static final TypeMetadata EMPTY
    pub fn empty() -> TypeMetadata {
        TypeMetadata {
            contents: BTreeMap::new(),
        }
    }

    /// Adapter for the Java identity check this == EMPTY.
    pub fn is_empty_metadata(&self) -> bool {
        self.contents.is_empty()
    }

    /// Java: public TypeMetadata(Entry elem)
    pub fn new_with(elem: Entry) -> TypeMetadata {
        let mut contents = BTreeMap::new();
        contents.insert(elem.kind(), elem);
        TypeMetadata { contents }
    }

    /// Java: public TypeMetadata(TypeMetadata other) — the copy constructor
    pub fn clone_of(other: &TypeMetadata) -> TypeMetadata {
        TypeMetadata {
            contents: other.contents.clone(),
        }
    }

    /// Java: public TypeMetadata combine(Entry elem)
    pub fn combine(&self, elem: Entry) -> TypeMetadata {
        let mut out = TypeMetadata::clone_of(self);
        let key = elem.kind();
        if let Some(existing) = self.contents.get(&key) {
            out.add(key, existing.clone().combine(elem));
        } else {
            out.add(key, elem);
        }
        out
    }

    /// Java: public TypeMetadata combineAll(TypeMetadata other)
    pub fn combine_all(&self, other: &TypeMetadata) -> TypeMetadata {
        let mut out = TypeMetadata::empty();
        let mut keys: Vec<EntryKind> = self.contents.keys().copied().collect();
        for key in other.contents.keys() {
            if !keys.contains(key) {
                keys.push(*key);
            }
        }
        for key in keys {
            match (self.contents.get(&key), other.contents.get(&key)) {
                (Some(left), Some(right)) => {
                    out.add(key, left.clone().combine(right.clone()));
                }
                (Some(left), None) => out.add(key, left.clone()),
                (None, Some(right)) => out.add(key, right.clone()),
                (None, None) => {}
            }
        }
        out
    }

    /// Java: public TypeMetadata without(Entry.Kind kind)
    pub fn without(&self, kind: EntryKind) -> TypeMetadata {
        if self.is_empty_metadata() || !self.contents.contains_key(&kind) {
            return TypeMetadata::clone_of(self);
        }
        let mut out = TypeMetadata::clone_of(self);
        out.contents.remove(&kind);
        if out.contents.is_empty() {
            TypeMetadata::empty()
        } else {
            out
        }
    }

    /// Java: public Entry get(Entry.Kind kind)
    pub fn get(&self, kind: EntryKind) -> Option<&Entry> {
        self.contents.get(&kind)
    }

    /// Java: private void add(Entry.Kind kind, Entry elem)
    fn add(&mut self, kind: EntryKind, elem: Entry) {
        self.contents.insert(kind, elem);
    }
}
