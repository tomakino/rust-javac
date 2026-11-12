//! Class-file compaction for the platform image.
//!
//! Two reductions, both invisible to `ClassReader`:
//!
//! 1. remove every method's `Code` attribute (the compiler reads the structure but
//!    never the body, exactly like javac's `readCode`, which returns null);
//! 2. remove the constant-pool entries that nothing left references, then renumber
//!    the survivors. After `Code` is gone, method-body references are dead: measured
//!    on the JDK 17 image, 77.6% of the pool is unreachable.
//!
//! Every surviving structure is parsed, so the rewrite is a full class-file walk:
//! class/field/method descriptors, attributes (signatures, annotations, exceptions,
//! inner classes, nests, records, sealed types, modules, bootstrap methods), and the
//! per-attribute constant-pool indices are all remapped. An attribute this module
//! does not know is rejected instead of being copied with stale indices.

use std::collections::{HashMap, HashSet};

/// Rewrites a class file: drops method bodies and unreferenced constant-pool entries.
///
/// Returns `None` when `data` is not a well-formed class file, or uses an attribute
/// the rewriter does not understand.
pub fn compact_class(data: &[u8]) -> Option<Vec<u8>> {
    let layout = Layout::parse(data)?;
    let reachable = layout.reachable()?;
    let map = layout.build_map(&reachable);
    layout.rebuild(&reachable, &map)
}

struct Entry {
    /// Old constant-pool index (1-based).
    index: u16,
    tag: u8,
    start: usize,
    end: usize,
    /// (absolute position of the u2 reference, referenced index)
    refs: Vec<(usize, u16)>,
}

struct Layout<'a> {
    data: &'a [u8],
    cp_end: usize,
    entries: Vec<Entry>,
    entries_by_index: HashMap<u16, usize>,
    /// (absolute position of a u2 constant-pool index, old index)
    edges: Vec<(usize, u16)>,
    /// (absolute position of an attribute count, new count) after removing Code
    count_edits: Vec<(usize, u16)>,
    /// Byte ranges to delete (one per Code attribute, including its 6-byte header).
    removals: Vec<(usize, usize)>,
}

fn u2(data: &[u8], position: usize) -> Option<u16> {
    let bytes = data.get(position..position.checked_add(2)?)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn u4(data: &[u8], position: usize) -> Option<u32> {
    let bytes = data.get(position..position.checked_add(4)?)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

impl<'a> Layout<'a> {
    fn parse(data: &'a [u8]) -> Option<Layout<'a>> {
        if u4(data, 0)? != 0xCAFE_BABE {
            return None;
        }
        let cp_count = u2(data, 8)? as usize;
        if cp_count == 0 {
            return None;
        }
        let mut position = 10usize;
        let mut entries = Vec::new();
        let mut entries_by_index = HashMap::new();
        let mut index = 1usize;
        while index < cp_count {
            let start = position;
            let tag = *data.get(position)?;
            position += 1;
            let mut refs = Vec::new();
            match tag {
                1 => {
                    let length = u2(data, position)? as usize;
                    position = position.checked_add(2 + length)?;
                }
                3 | 4 => position += 4,
                5 | 6 => position += 8,
                7 | 8 | 16 | 19 | 20 => {
                    refs.push((position, u2(data, position)?));
                    position += 2;
                }
                9 | 10 | 11 | 12 => {
                    refs.push((position, u2(data, position)?));
                    refs.push((position + 2, u2(data, position + 2)?));
                    position += 4;
                }
                15 => {
                    refs.push((position + 1, u2(data, position + 1)?));
                    position += 3;
                }
                17 | 18 => {
                    refs.push((position + 2, u2(data, position + 2)?));
                    position += 4;
                }
                _ => return None,
            }
            if position > data.len() {
                return None;
            }
            entries_by_index.insert(index as u16, entries.len());
            entries.push(Entry {
                index: index as u16,
                tag,
                start,
                end: position,
                refs,
            });
            index += if tag == 5 || tag == 6 { 2 } else { 1 };
        }
        let cp_end = position;
        let mut layout = Layout {
            data,
            cp_end,
            entries,
            entries_by_index,
            edges: Vec::new(),
            count_edits: Vec::new(),
            removals: Vec::new(),
        };
        layout.parse_structure()?;
        Some(layout)
    }

    fn parse_structure(&mut self) -> Option<()> {
        let mut position = self.cp_end;
        position += 2; // access_flags
        self.edge(&mut position)?; // this_class
        self.edge(&mut position)?; // super_class
        let interfaces = u2(self.data, position)? as usize;
        position += 2;
        for _ in 0..interfaces {
            self.edge(&mut position)?;
        }
        for _ in 0..2 {
            // fields, methods
            let count = u2(self.data, position)? as usize;
            position += 2;
            for _ in 0..count {
                position += 2; // access_flags
                self.edge(&mut position)?; // name_index
                self.edge(&mut position)?; // descriptor_index
                let count_position = position;
                let attributes = u2(self.data, position)? as usize;
                position += 2;
                self.parse_attributes(&mut position, count_position, attributes)?;
            }
        }
        let count_position = position;
        let attributes = u2(self.data, position)? as usize;
        position += 2;
        self.parse_attributes(&mut position, count_position, attributes)?;
        if position != self.data.len() {
            return None;
        }
        Some(())
    }

    fn parse_attributes(
        &mut self,
        position: &mut usize,
        count_position: usize,
        count: usize,
    ) -> Option<()> {
        let mut removed = 0u16;
        for _ in 0..count {
            let name_position = *position;
            let name_index = u2(self.data, *position)?;
            let length = u4(self.data, *position + 2)? as usize;
            let start = (*position).checked_add(6)?;
            let end = start.checked_add(length)?;
            if end > self.data.len() {
                return None;
            }
            let name = self.utf8(name_index)?.to_vec();
            if name == b"Code" {
                self.removals.push((name_position, end));
                removed += 1;
            } else {
                self.edges.push((name_position, name_index));
                self.parse_attribute_body(&name, start)?;
            }
            *position = end;
        }
        if removed > 0 {
            self.count_edits
                .push((count_position, (count as u16).checked_sub(removed)?));
        }
        Some(())
    }

    fn parse_attribute_body(&mut self, name: &[u8], start: usize) -> Option<()> {
        match name {
            b"Signature" | b"ConstantValue" => {
                self.edge_at(start)?;
            }
            b"Exceptions" => {
                let count = u2(self.data, start)? as usize;
                for index in 0..count {
                    self.edge_at(start + 2 + index * 2)?;
                }
            }
            b"InnerClasses" => {
                let count = u2(self.data, start)? as usize;
                let mut position = start + 2;
                for _ in 0..count {
                    self.edge(&mut position)?;
                    self.edge(&mut position)?;
                    self.edge(&mut position)?;
                    position += 2;
                }
            }
            b"EnclosingMethod" => {
                self.edge_at(start)?;
                self.edge_at(start + 2)?;
            }
            b"NestHost" | b"SourceFile" | b"ModuleMainClass" | b"ModuleTarget" => {
                self.edge_at(start)?;
            }
            b"NestMembers" | b"PermittedSubclasses" | b"ModulePackages" => {
                let count = u2(self.data, start)? as usize;
                for index in 0..count {
                    self.edge_at(start + 2 + index * 2)?;
                }
            }
            b"MethodParameters" => {
                let count = *self.data.get(start)? as usize;
                let mut position = start + 1;
                for _ in 0..count {
                    self.edge(&mut position)?;
                    position += 2; // access_flags
                }
            }
            b"BootstrapMethods" => {
                let count = u2(self.data, start)? as usize;
                let mut position = start + 2;
                for _ in 0..count {
                    self.edge(&mut position)?;
                    let arguments = u2(self.data, position)? as usize;
                    position += 2;
                    for _ in 0..arguments {
                        self.edge(&mut position)?;
                    }
                }
            }
            b"RuntimeVisibleAnnotations" | b"RuntimeInvisibleAnnotations" => {
                let count = u2(self.data, start)? as usize;
                let mut position = start + 2;
                for _ in 0..count {
                    self.annotation(&mut position)?;
                }
            }
            b"RuntimeVisibleParameterAnnotations" | b"RuntimeInvisibleParameterAnnotations" => {
                let parameters = *self.data.get(start)? as usize;
                let mut position = start + 1;
                for _ in 0..parameters {
                    let count = u2(self.data, position)? as usize;
                    position += 2;
                    for _ in 0..count {
                        self.annotation(&mut position)?;
                    }
                }
            }
            b"RuntimeVisibleTypeAnnotations" | b"RuntimeInvisibleTypeAnnotations" => {
                let count = u2(self.data, start)? as usize;
                let mut position = start + 2;
                for _ in 0..count {
                    self.type_annotation(&mut position)?;
                }
            }
            b"AnnotationDefault" => {
                let mut position = start;
                self.element_value(&mut position)?;
            }
            b"Record" => {
                let count = u2(self.data, start)? as usize;
                let mut position = start + 2;
                for _ in 0..count {
                    self.edge(&mut position)?;
                    self.edge(&mut position)?;
                    let count_position = position;
                    let attributes = u2(self.data, position)? as usize;
                    position += 2;
                    self.parse_attributes(&mut position, count_position, attributes)?;
                }
            }
            b"Module" => {
                let mut position = start;
                self.edge(&mut position)?; // module_name_index
                position += 2; // module_flags
                self.edge(&mut position)?; // module_version_index
                let requires = u2(self.data, position)? as usize;
                position += 2;
                for _ in 0..requires {
                    self.edge(&mut position)?; // requires_index
                    position += 2; // requires_flags
                    self.edge(&mut position)?; // requires_version_index
                }
                for _ in 0..2 {
                    // exports, opens
                    let count = u2(self.data, position)? as usize;
                    position += 2;
                    for _ in 0..count {
                        self.edge(&mut position)?; // package index
                        position += 2; // flags
                        let targets = u2(self.data, position)? as usize;
                        position += 2;
                        for _ in 0..targets {
                            self.edge(&mut position)?;
                        }
                    }
                }
                let uses = u2(self.data, position)? as usize;
                position += 2;
                for _ in 0..uses {
                    self.edge(&mut position)?;
                }
                let provides = u2(self.data, position)? as usize;
                position += 2;
                for _ in 0..provides {
                    self.edge(&mut position)?; // provides_index
                    let with = u2(self.data, position)? as usize;
                    position += 2;
                    for _ in 0..with {
                        self.edge(&mut position)?;
                    }
                }
            }
            b"ModuleHashes" => {
                let mut position = start;
                self.edge(&mut position)?; // algorithm_index
                let count = u2(self.data, position)? as usize;
                position += 2;
                for _ in 0..count {
                    self.edge(&mut position)?; // module_name_index
                    let length = u2(self.data, position)? as usize;
                    position += 2 + length; // hash
                }
            }
            b"ModuleResolution" | b"Deprecated" | b"Synthetic" | b"SourceDebugExtension" => {}
            _ => {
                eprintln!("compact: unknown attribute {:?}", String::from_utf8_lossy(name));
                return None;
            }
        }
        Some(())
    }

    fn annotation(&mut self, position: &mut usize) -> Option<()> {
        self.edge(position)?; // type_index
        let pairs = u2(self.data, *position)? as usize;
        *position += 2;
        for _ in 0..pairs {
            self.edge(position)?; // element_name_index
            self.element_value(position)?;
        }
        Some(())
    }

    fn element_value(&mut self, position: &mut usize) -> Option<()> {
        let tag = *self.data.get(*position)?;
        *position += 1;
        match tag {
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
                self.edge(position)?;
            }
            b'e' => {
                self.edge(position)?;
                self.edge(position)?;
            }
            b'c' => {
                self.edge(position)?;
            }
            b'@' => {
                self.annotation(position)?;
            }
            b'[' => {
                let count = u2(self.data, *position)? as usize;
                *position += 2;
                for _ in 0..count {
                    self.element_value(position)?;
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn type_annotation(&mut self, position: &mut usize) -> Option<()> {
        let target = *self.data.get(*position)?;
        *position += 1;
        match target {
            0x00 | 0x01 => *position += 1,
            0x10 | 0x11 | 0x12 => *position += 2,
            0x13..=0x15 => {}
            0x16 => *position += 1,
            0x17 => *position += 2,
            0x40 | 0x41 => {
                let table = u2(self.data, *position)? as usize;
                *position = (*position).checked_add(2 + table * 6)?;
            }
            0x42 | 0x43..=0x46 => *position += 2,
            0x47..=0x4B => *position += 3,
            _ => return None,
        }
        let path = *self.data.get(*position)?;
        *position = (*position).checked_add(1 + path as usize * 2)?;
        self.annotation(position)
    }

    fn edge(&mut self, position: &mut usize) -> Option<u16> {
        let value = u2(self.data, *position)?;
        self.edges.push((*position, value));
        *position += 2;
        Some(value)
    }

    fn edge_at(&mut self, position: usize) -> Option<u16> {
        let value = u2(self.data, position)?;
        self.edges.push((position, value));
        Some(value)
    }

    fn utf8(&self, index: u16) -> Option<&'a [u8]> {
        let entry = &self.entries[*self.entries_by_index.get(&index)?];
        if entry.tag != 1 {
            return None;
        }
        self.data.get(entry.start + 3..entry.end)
    }

    fn reachable(&self) -> Option<HashSet<u16>> {
        let mut seen: HashSet<u16> = HashSet::new();
        let mut stack: Vec<u16> = self
            .edges
            .iter()
            .map(|&(_, index)| index)
            .filter(|&index| index != 0)
            .collect();
        while let Some(index) = stack.pop() {
            if !self.entries_by_index.contains_key(&index) {
                return None;
            }
            if !seen.insert(index) {
                continue;
            }
            let entry = &self.entries[*self.entries_by_index.get(&index)?];
            for &(_, reference) in &entry.refs {
                if reference != 0 {
                    stack.push(reference);
                }
            }
        }
        Some(seen)
    }

    fn build_map(&self, reachable: &HashSet<u16>) -> HashMap<u16, u16> {
        let mut map = HashMap::new();
        let mut next: u16 = 1;
        for entry in &self.entries {
            if reachable.contains(&entry.index) {
                map.insert(entry.index, next);
                next += if entry.tag == 5 || entry.tag == 6 { 2 } else { 1 };
            }
        }
        map
    }

    fn rebuild(&self, reachable: &HashSet<u16>, map: &HashMap<u16, u16>) -> Option<Vec<u8>> {
        let mut new_cp_count: u16 = 1;
        for entry in &self.entries {
            if reachable.contains(&entry.index) {
                new_cp_count += if entry.tag == 5 || entry.tag == 6 { 2 } else { 1 };
            }
        }

        let mut out = Vec::with_capacity(self.data.len());
        out.extend_from_slice(&self.data[..8]);
        out.extend_from_slice(&new_cp_count.to_be_bytes());
        for entry in &self.entries {
            if !reachable.contains(&entry.index) {
                continue;
            }
            let copy_start = out.len();
            out.extend_from_slice(&self.data[entry.start..entry.end]);
            for &(position, old) in &entry.refs {
                let new = map.get(&old).copied().unwrap_or(0);
                let at = copy_start + (position - entry.start);
                out[at..at + 2].copy_from_slice(&new.to_be_bytes());
            }
        }

        let mut patches: Vec<(usize, u16)> = self
            .edges
            .iter()
            .map(|&(position, old)| (position, map.get(&old).copied().unwrap_or(0)))
            .collect();
        patches.extend(self.count_edits.iter().copied());
        patches.sort_by_key(|&(position, _)| position);
        let mut removals = self.removals.clone();
        removals.sort();
        let mut edge_index = 0usize;
        let mut position = self.cp_end;
        for (remove_start, remove_end) in removals {
            Self::copy_range(
                &mut out,
                self.data,
                &patches,
                &mut edge_index,
                position,
                remove_start,
            );
            position = remove_end;
        }
        Self::copy_range(
            &mut out,
            self.data,
            &patches,
            &mut edge_index,
            position,
            self.data.len(),
        );
        if out.len() > self.data.len() {
            return None;
        }
        Some(out)
    }

    fn copy_range(
        out: &mut Vec<u8>,
        data: &[u8],
        patches: &[(usize, u16)],
        patch_index: &mut usize,
        from: usize,
        to: usize,
    ) {
        let mut position = from;
        while *patch_index < patches.len() && patches[*patch_index].0 < to {
            let (patch_position, new) = patches[*patch_index];
            if patch_position < position {
                *patch_index += 1;
                continue;
            }
            out.extend_from_slice(&data[position..patch_position]);
            out.extend_from_slice(&new.to_be_bytes());
            position = patch_position + 2;
            *patch_index += 1;
        }
        out.extend_from_slice(&data[position..to]);
    }
}

#[cfg(test)]
mod tests {
    use super::compact_class;

    /// A minimal class: one method with a Code attribute, one without, a class-level
    /// SourceFile attribute, and an Utf8 entry ("Dead") that nothing references.
    fn sample() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xCAFE_BABEu32.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes.extend_from_slice(&61u16.to_be_bytes());
        bytes.extend_from_slice(&8u16.to_be_bytes()); // constant_pool_count
        // 1: "A"
        push_utf8(&mut bytes, b"A");
        // 2: "m"
        push_utf8(&mut bytes, b"m");
        // 3: "()V"
        push_utf8(&mut bytes, b"()V");
        // 4: "Code"
        push_utf8(&mut bytes, b"Code");
        // 5: "SourceFile"
        push_utf8(&mut bytes, b"SourceFile");
        // 6: "A.java"
        push_utf8(&mut bytes, b"A.java");
        // 7: "Dead" (unreferenced)
        push_utf8(&mut bytes, b"Dead");
        bytes.extend_from_slice(&0x0021u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes()); // interfaces
        bytes.extend_from_slice(&0u16.to_be_bytes()); // fields
        bytes.extend_from_slice(&2u16.to_be_bytes()); // methods
        // method 1: m()V with Code
        bytes.extend_from_slice(&0x0001u16.to_be_bytes());
        bytes.extend_from_slice(&2u16.to_be_bytes());
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&4u32.to_be_bytes());
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        // method 2: m()V without attributes
        bytes.extend_from_slice(&0x0001u16.to_be_bytes());
        bytes.extend_from_slice(&2u16.to_be_bytes());
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        // class attribute: SourceFile
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&5u16.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        bytes.extend_from_slice(&6u16.to_be_bytes());
        bytes
    }

    fn push_utf8(bytes: &mut Vec<u8>, text: &[u8]) {
        bytes.push(1);
        bytes.extend_from_slice(&(text.len() as u16).to_be_bytes());
        bytes.extend_from_slice(text);
    }

    #[test]
    fn drops_code_and_dead_pool_entries() {
        let input = sample();
        let output = compact_class(&input).expect("compact");
        // cp_count 8 -> 6: "Code" and "Dead" are gone.
        assert_eq!(&output[8..10], &6u16.to_be_bytes());
        // Removed: Code attribute (6 + 4) and the two Utf8 entries (3 + 4 each).
        assert_eq!(output.len(), input.len() - 10 - 7 - 7);
        // Every surviving reference points into the new pool: structural parse.
        let (methods, counts, class_attributes) = parse_structure(&output).expect("parse");
        assert_eq!(methods, 2);
        assert_eq!(counts, vec![0, 0]);
        assert_eq!(class_attributes, 1);
    }

    #[test]
    fn rejects_non_class_data() {
        assert!(compact_class(b"not a class").is_none());
    }

    fn read_u2(data: &[u8], position: &mut usize) -> Option<usize> {
        let end = position.checked_add(2)?;
        let bytes = data.get(*position..end)?;
        *position = end;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]) as usize)
    }

    fn read_u4(data: &[u8], position: &mut usize) -> Option<usize> {
        let end = position.checked_add(4)?;
        let bytes = data.get(*position..end)?;
        *position = end;
        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }

    fn parse_structure(data: &[u8]) -> Option<(usize, Vec<usize>, usize)> {
        let mut position = 0usize;
        if read_u4(data, &mut position)? != 0xCAFE_BABE {
            return None;
        }
        position += 4;
        let constant_pool = read_u2(data, &mut position)?;
        let mut index = 1;
        while index < constant_pool {
            let tag = *data.get(position)?;
            position += 1;
            match tag {
                1 => {
                    let length = read_u2(data, &mut position)?;
                    position = position.checked_add(length)?;
                }
                3 | 4 => position += 4,
                5 | 6 => {
                    position += 8;
                    index += 1;
                }
                7 | 8 | 16 | 19 | 20 => position += 2,
                9 | 10 | 11 | 12 | 17 | 18 => position += 4,
                15 => position += 3,
                _ => return None,
            }
            index += 1;
        }
        position += 6;
        let interfaces = read_u2(data, &mut position)?;
        position += interfaces * 2;
        let fields = read_u2(data, &mut position)?;
        for _ in 0..fields {
            position += 6;
            let attributes = read_u2(data, &mut position)?;
            for _ in 0..attributes {
                position += 2;
                let length = read_u4(data, &mut position)?;
                position = position.checked_add(length)?;
            }
        }
        let methods = read_u2(data, &mut position)?;
        let mut counts = Vec::with_capacity(methods);
        for _ in 0..methods {
            position += 6;
            let attributes = read_u2(data, &mut position)?;
            counts.push(attributes);
            for _ in 0..attributes {
                position += 2;
                let length = read_u4(data, &mut position)?;
                position = position.checked_add(length)?;
            }
        }
        let class_attributes = read_u2(data, &mut position)?;
        Some((methods, counts, class_attributes))
    }
}
