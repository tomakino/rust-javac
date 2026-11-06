//! Interned identities used by the project index.
//!
//! Every index reference is a compact `u32` id; the strings live in a single
//! [`StringInterner`] so symbol keys compare without allocating.

use std::collections::HashMap;

pub type FileId = u32;
pub type SymbolId = u32;
pub type NameId = u32;
pub type SignatureId = u32;

/// The only string table in the index. Names and signature fingerprints are
/// interned here; the index never stores `String` in hot structures.
#[derive(Debug, Default, Clone)]
pub struct StringInterner {
    strings: Vec<String>,
    ids: HashMap<String, u32>,
}

impl StringInterner {
    pub fn intern(&mut self, value: &str) -> u32 {
        if let Some(id) = self.ids.get(value) {
            return *id;
        }
        let id = self.strings.len() as u32;
        let owned = value.to_string();
        self.strings.push(owned.clone());
        self.ids.insert(owned, id);
        id
    }

    pub fn get(&self, id: u32) -> &str {
        self.strings
            .get(id as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Lookup without interning; used by query paths that must not mutate.
    pub fn find(&self, value: &str) -> Option<u32> {
        self.ids.get(value).copied()
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> {
        self.strings
            .iter()
            .enumerate()
            .map(|(id, text)| (id as u32, text.as_str()))
    }

    /// Restore a table that was serialized to disk.
    pub fn from_strings(strings: Vec<String>) -> Self {
        let mut ids = HashMap::with_capacity(strings.len());
        for (id, text) in strings.iter().enumerate() {
            ids.insert(text.clone(), id as u32);
        }
        Self { strings, ids }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexSymbolKind {
    Class,
    Interface,
    Enum,
    Annotation,
    Record,
    Field,
    Method,
    Constructor,
    Local,
    Parameter,
    TypeParameter,
    Other,
}

impl IndexSymbolKind {
    pub fn is_type(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Interface | Self::Enum | Self::Annotation | Self::Record
        )
    }

    pub fn is_member(self) -> bool {
        matches!(self, Self::Field | Self::Method | Self::Constructor)
    }

    /// Stable on-disk tag. Never reorder without bumping the schema version.
    pub fn tag(self) -> u8 {
        match self {
            Self::Class => 1,
            Self::Interface => 2,
            Self::Enum => 3,
            Self::Annotation => 4,
            Self::Record => 5,
            Self::Field => 6,
            Self::Method => 7,
            Self::Constructor => 8,
            Self::Local => 9,
            Self::Parameter => 10,
            Self::TypeParameter => 11,
            Self::Other => 12,
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Class,
            2 => Self::Interface,
            3 => Self::Enum,
            4 => Self::Annotation,
            5 => Self::Record,
            6 => Self::Field,
            7 => Self::Method,
            8 => Self::Constructor,
            9 => Self::Local,
            10 => Self::Parameter,
            11 => Self::TypeParameter,
            12 => Self::Other,
            _ => return None,
        })
    }
}

/// Identity of a declaration inside the project.
///
/// - types: `owner` + `name` + `kind`
/// - fields: `owner` + `name`
/// - methods/constructors: `owner` + `name` + parameter fingerprint + arity
/// - locals/parameters: `signature` carries `file:position:index`; they are
///   only ever referenced from the declaring file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolKey {
    pub owner: Option<SymbolId>,
    pub name: NameId,
    pub kind: IndexSymbolKind,
    pub signature: SignatureId,
}

#[derive(Debug, Default)]
pub struct StringInternerBuilder {
    inner: StringInterner,
}
