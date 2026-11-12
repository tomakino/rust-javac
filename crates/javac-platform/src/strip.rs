//! Removes the `Code` attribute from class files for the platform image.
//!
//! The compiler reads class files through `ClassReader`, which reads the `Code`
//! structure but never the method body (`read_code` returns null, exactly like javac).
//! The attribute is therefore pure payload: dropping it shrinks the image without
//! changing anything the compiler observes. The constant pool and every other
//! attribute are copied byte for byte, so no index needs rewriting.

use std::collections::HashSet;

/// Returns `data` with every method's `Code` attribute removed.
///
/// Returns `None` when `data` is not a well-formed class file.
pub fn strip_code(data: &[u8]) -> Option<Vec<u8>> {
    let mut reader = Reader::new(data);
    if reader.u4()? != 0xCAFE_BABE {
        return None;
    }
    reader.skip(4)?; // minor_version, major_version

    let constant_pool_count = reader.u2()? as usize;
    let mut code_names: HashSet<u16> = HashSet::new();
    let mut index = 1usize;
    while index < constant_pool_count {
        match reader.u1()? {
            1 => {
                let length = reader.u2()? as usize;
                let bytes = reader.take(length)?;
                if bytes == b"Code" {
                    code_names.insert(index as u16);
                }
            }
            3 | 4 => reader.skip(4)?,
            5 | 6 => {
                reader.skip(8)?;
                index += 1;
            }
            7 | 8 | 16 | 19 | 20 => reader.skip(2)?,
            9 | 10 | 11 | 12 | 17 | 18 => reader.skip(4)?,
            15 => reader.skip(3)?,
            _ => return None,
        }
        index += 1;
    }
    if code_names.is_empty() {
        return Some(data.to_vec());
    }

    reader.skip(6)?; // access_flags, this_class, super_class
    let interfaces = reader.u2()? as usize;
    reader.skip(interfaces * 2)?;

    let fields = reader.u2()? as usize;
    for _ in 0..fields {
        reader.skip(6)?; // access_flags, name_index, descriptor_index
        skip_attributes(&mut reader)?;
    }

    let methods_position = reader.position();
    let methods = reader.u2()? as usize;
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..methods_position]);
    out.extend_from_slice(&(methods as u16).to_be_bytes());

    for _ in 0..methods {
        let start = reader.position();
        reader.skip(6)?; // access_flags, name_index, descriptor_index
        let attribute_count = reader.u2()? as usize;
        out.extend_from_slice(&data[start..start + 6]);

        let mut kept: Vec<(u16, u32, usize)> = Vec::with_capacity(attribute_count);
        for _ in 0..attribute_count {
            let name_index = reader.u2()?;
            let length = reader.u4()? as usize;
            let data_start = reader.position();
            reader.skip(length)?;
            if !code_names.contains(&name_index) {
                kept.push((name_index, length as u32, data_start));
            }
        }
        out.extend_from_slice(&(kept.len() as u16).to_be_bytes());
        for (name_index, length, data_start) in kept {
            out.extend_from_slice(&name_index.to_be_bytes());
            out.extend_from_slice(&length.to_be_bytes());
            out.extend_from_slice(&data[data_start..data_start + length as usize]);
        }
    }

    // Class attributes (SourceFile, InnerClasses, NestHost, Record, Module, ...) are
    // copied verbatim, including the attribute count.
    out.extend_from_slice(&data[reader.position()..]);
    Some(out)
}

fn skip_attributes(reader: &mut Reader<'_>) -> Option<()> {
    let count = reader.u2()? as usize;
    for _ in 0..count {
        reader.skip(2)?; // attribute_name_index
        let length = reader.u4()? as usize;
        reader.skip(length)?;
    }
    Some(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, position: 0 }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn u1(&mut self) -> Option<u8> {
        let value = *self.bytes.get(self.position)?;
        self.position += 1;
        Some(value)
    }

    fn u2(&mut self) -> Option<u16> {
        let bytes = self.take(2)?;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u4(&mut self) -> Option<u32> {
        let bytes = self.take(4)?;
        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(length)?;
        let slice = self.bytes.get(self.position..end)?;
        self.position = end;
        Some(slice)
    }

    fn skip(&mut self, length: usize) -> Option<()> {
        self.take(length).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::strip_code;

    /// A minimal class file: one method with a Code attribute and one without, plus a
    /// class-level SourceFile attribute.
    fn sample() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xCAFE_BABEu32.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes()); // minor
        bytes.extend_from_slice(&61u16.to_be_bytes()); // major
        bytes.extend_from_slice(&7u16.to_be_bytes()); // constant_pool_count
        // 1: "A" (class name)
        bytes.push(1);
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(b'A');
        // 2: "m" (method name)
        bytes.push(1);
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(b'm');
        // 3: "()V"
        bytes.push(1);
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(b"()V");
        // 4: "Code"
        bytes.push(1);
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(b"Code");
        // 5: "SourceFile"
        bytes.push(1);
        bytes.extend_from_slice(&10u16.to_be_bytes());
        bytes.extend_from_slice(b"SourceFile");
        // 6: "A.java"
        bytes.push(1);
        bytes.extend_from_slice(&6u16.to_be_bytes());
        bytes.extend_from_slice(b"A.java");
        bytes.extend_from_slice(&0x0021u16.to_be_bytes()); // access
        bytes.extend_from_slice(&1u16.to_be_bytes()); // this
        bytes.extend_from_slice(&1u16.to_be_bytes()); // super
        bytes.extend_from_slice(&0u16.to_be_bytes()); // interfaces
        bytes.extend_from_slice(&0u16.to_be_bytes()); // fields
        bytes.extend_from_slice(&2u16.to_be_bytes()); // methods
        // method 1: m()V with Code
        bytes.extend_from_slice(&0x0001u16.to_be_bytes());
        bytes.extend_from_slice(&2u16.to_be_bytes());
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes()); // Code
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

    #[test]
    fn removes_code_and_keeps_everything_else() {
        let input = sample();
        let output = strip_code(&input).expect("strip");
        // The Code attribute header (6 bytes) plus its 4 bytes of data are gone.
        assert_eq!(output.len(), input.len() - 10);
        // No method attribute entry names Code any more.
        assert!(
            !output
                .windows(6)
                .any(|window| window.iter().eq([0u8, 4, 0, 0, 0, 4].iter())),
            "Code attribute entry still present"
        );
        // The class-level SourceFile attribute (count 1, name index 5) is intact.
        let tail = &output[output.len() - 10..];
        assert_eq!(&tail[..2], &1u16.to_be_bytes());
        assert_eq!(&tail[2..4], &5u16.to_be_bytes());
        // Re-parse the output: both methods lost their only attribute.
        let (methods, counts, class_attributes) =
            parse_structure(&output).expect("well-formed output");
        assert_eq!(methods, 2);
        assert_eq!(counts, vec![0, 0]);
        assert_eq!(class_attributes, 1);
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

    /// Returns (method count, attributes per method, class attribute count).
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

    #[test]
    fn rejects_non_class_data() {
        assert!(strip_code(b"not a class").is_none());
    }
}
