// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.Buffer / ByteBuffer / CharBuffer and
// java.nio.charset.CoderResult / CodingErrorAction, the subset used by
// com.sun.tools.javac.file.BaseFileManager and PathFileObject.
// Why not 1:1: java.nio buffers are native-memory ByteBuffer/CharBuffer
// implementations; the port needs the observable buffer semantics
// (position/limit/capacity/remaining/flip/clear/get/put) so this module
// keeps those operations on Vec-backed buffers.  The CoderResult values are
// the exact java.nio.charset CoderResult states the Javac decoder loop
// distinguishes (underflow/overflow/malformed/unmappable + length).

//! Minimal java.nio buffer and CoderResult semantics for the charset layer.

use javac_strings::JavaString;

/// Java: java.nio.charset.CodingErrorAction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodingErrorAction {
    /// Java: CodingErrorAction.REPORT.
    Report,
    /// Java: CodingErrorAction.REPLACE.
    Replace,
    /// Java: CodingErrorAction.IGNORE.
    Ignore,
}

/// Java: java.nio.charset.CoderResult.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoderResult {
    /// Java: CoderResult.UNDERFLOW.
    Underflow,
    /// Java: CoderResult.OVERFLOW.
    Overflow,
    /// Java: CoderResult.malformedForLength(int).
    MalformedForLength(usize),
    /// Java: CoderResult.unmappableForLength(int).
    UnmappableForLength(usize),
}

impl CoderResult {
    /// Java: public boolean isUnderflow().
    ///
    /// # Returns
    ///   - true for UNDERFLOW.
    pub fn is_underflow(self) -> bool {
        matches!(self, CoderResult::Underflow)
    }

    /// Java: public boolean isOverflow().
    ///
    /// # Returns
    ///   - true for OVERFLOW.
    pub fn is_overflow(self) -> bool {
        matches!(self, CoderResult::Overflow)
    }

    /// Java: public boolean isMalformed().
    ///
    /// # Returns
    ///   - true for a malformed-input result.
    pub fn is_malformed(self) -> bool {
        matches!(self, CoderResult::MalformedForLength(_))
    }

    /// Java: public boolean isUnmappable().
    ///
    /// # Returns
    ///   - true for an unmappable-character result.
    pub fn is_unmappable(self) -> bool {
        matches!(self, CoderResult::UnmappableForLength(_))
    }

    /// Java: public int length().
    ///
    /// # Returns
    ///   - the malformed/unmappable length (0 for underflow/overflow).
    pub fn length(self) -> usize {
        match self {
            CoderResult::MalformedForLength(length) | CoderResult::UnmappableForLength(length) => {
                length
            }
            _ => 0,
        }
    }
}

impl std::fmt::Display for CoderResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoderResult::Underflow => write!(f, "UNDERFLOW"),
            CoderResult::Overflow => write!(f, "OVERFLOW"),
            CoderResult::MalformedForLength(length) => write!(f, "MALFORMED[{length}]"),
            CoderResult::UnmappableForLength(length) => write!(f, "UNMAPPABLE[{length}]"),
        }
    }
}

/// Java: java.nio.ByteBuffer (the position/limit/capacity view).
#[derive(Clone, Debug)]
pub struct ByteBuffer {
    data: Vec<u8>,
    position: usize,
    limit: usize,
}

impl ByteBuffer {
    /// Java: public static ByteBuffer allocate(int capacity).
    ///
    /// # Arguments
    ///   - capacity: the buffer capacity.
    ///
    /// # Returns
    ///   - a zero-filled buffer with position 0 and limit capacity.
    pub fn allocate(capacity: usize) -> ByteBuffer {
        ByteBuffer {
            data: vec![0; capacity],
            position: 0,
            limit: capacity,
        }
    }

    /// Java: public static ByteBuffer wrap(byte[] array).
    ///
    /// # Arguments
    ///   - bytes: the wrapped bytes.
    ///
    /// # Returns
    ///   - a buffer with position 0 and limit array length.
    pub fn wrap(bytes: &[u8]) -> ByteBuffer {
        ByteBuffer {
            data: bytes.to_vec(),
            position: 0,
            limit: bytes.len(),
        }
    }

    /// Java: public final int capacity().
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    /// Java: public final int position().
    pub fn position(&self) -> usize {
        self.position
    }

    /// Java: public final Buffer position(int newPosition).
    pub fn set_position(&mut self, position: usize) -> &mut ByteBuffer {
        assert!(
            position <= self.limit,
            "IllegalArgumentException: position > limit"
        );
        self.position = position;
        self
    }

    /// Java: public final int limit().
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Java: public final Buffer limit(int newLimit).
    pub fn set_limit(&mut self, limit: usize) -> &mut ByteBuffer {
        assert!(
            limit <= self.capacity(),
            "IllegalArgumentException: limit > capacity"
        );
        self.limit = limit;
        if self.position > self.limit {
            self.position = self.limit;
        }
        self
    }

    /// Java: public final int remaining().
    pub fn remaining(&self) -> usize {
        self.limit - self.position
    }

    /// Java: public final boolean hasRemaining().
    pub fn has_remaining(&self) -> bool {
        self.position < self.limit
    }

    /// Java: public ByteBuffer flip().
    pub fn flip(&mut self) -> &mut ByteBuffer {
        self.limit = self.position;
        self.position = 0;
        self
    }

    /// Java: public ByteBuffer clear().
    pub fn clear(&mut self) -> &mut ByteBuffer {
        self.position = 0;
        self.limit = self.capacity();
        self
    }

    /// Java: public byte get().
    ///
    /// # Returns
    ///   - the byte at the current position (position advances).
    pub fn get(&mut self) -> u8 {
        assert!(self.has_remaining(), "BufferUnderflowException");
        let byte = self.data[self.position];
        self.position += 1;
        byte
    }

    /// Java: public byte get(int index).
    pub fn get_at(&self, index: usize) -> u8 {
        self.data[index]
    }

    /// Java: public final ByteBuffer put(byte b).
    pub fn put(&mut self, byte: u8) -> &mut ByteBuffer {
        assert!(self.position < self.limit, "BufferOverflowException");
        self.data[self.position] = byte;
        self.position += 1;
        self
    }

    /// Java: public ByteBuffer put(byte[] src) (whole array).
    pub fn put_slice(&mut self, bytes: &[u8]) -> &mut ByteBuffer {
        for byte in bytes {
            self.put(*byte);
        }
        self
    }

    /// Advance the position by the consumed byte count (CoderResult
    /// bookkeeping; the Java loop advances the input buffer by the consumed
    /// sequence length).
    pub fn advance(&mut self, count: usize) {
        assert!(
            self.position + count <= self.limit,
            "BufferUnderflowException"
        );
        self.position += count;
    }

    /// The bytes between position and limit.
    pub fn remaining_slice(&self) -> &[u8] {
        &self.data[self.position..self.limit]
    }

    /// The bytes between index 0 and limit.
    pub fn slice_upto_limit(&self) -> &[u8] {
        &self.data[..self.limit]
    }

    /// Replace the backing bytes (used by the BaseFileManager adapter when it
    /// copies a flipped buffer into a larger one).
    pub fn put_flipped(&mut self, other: &ByteBuffer) -> &mut ByteBuffer {
        self.put_slice(&other.data[..other.limit]);
        self
    }
}

/// Java: java.nio.CharBuffer (the position/limit/capacity view).
#[derive(Clone, Debug)]
pub struct CharBuffer {
    data: Vec<u16>,
    position: usize,
    limit: usize,
}

impl CharBuffer {
    /// Java: public static CharBuffer allocate(int capacity).
    pub fn allocate(capacity: usize) -> CharBuffer {
        CharBuffer {
            data: vec![0; capacity],
            position: 0,
            limit: capacity,
        }
    }

    /// Java: public static CharBuffer wrap(CharSequence) (the javac use).
    pub fn wrap_units(units: &[u16]) -> CharBuffer {
        CharBuffer {
            data: units.to_vec(),
            position: 0,
            limit: units.len(),
        }
    }

    /// Java: public final int capacity().
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    /// Java: public final int position().
    pub fn position(&self) -> usize {
        self.position
    }

    /// Java: public final Buffer position(int newPosition).
    pub fn set_position(&mut self, position: usize) -> &mut CharBuffer {
        assert!(
            position <= self.limit,
            "IllegalArgumentException: position > limit"
        );
        self.position = position;
        self
    }

    /// Java: public final int limit().
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Java: public final Buffer limit(int newLimit).
    pub fn set_limit(&mut self, limit: usize) -> &mut CharBuffer {
        assert!(
            limit <= self.capacity(),
            "IllegalArgumentException: limit > capacity"
        );
        self.limit = limit;
        if self.position > self.limit {
            self.position = self.limit;
        }
        self
    }

    /// Java: public final int remaining().
    pub fn remaining(&self) -> usize {
        self.limit - self.position
    }

    /// Java: public final boolean hasRemaining().
    pub fn has_remaining(&self) -> bool {
        self.position < self.limit
    }

    /// Java: public CharBuffer flip().
    pub fn flip(&mut self) -> &mut CharBuffer {
        self.limit = self.position;
        self.position = 0;
        self
    }

    /// Java: public CharBuffer clear().
    pub fn clear(&mut self) -> &mut CharBuffer {
        self.position = 0;
        self.limit = self.capacity();
        self
    }

    /// Java: public CharBuffer put(char c).
    pub fn put(&mut self, unit: u16) -> &mut CharBuffer {
        assert!(self.position < self.limit, "BufferOverflowException");
        self.data[self.position] = unit;
        self.position += 1;
        self
    }

    /// Java: public CharBuffer put(char[] src).
    pub fn put_units(&mut self, units: &[u16]) -> &mut CharBuffer {
        for unit in units {
            self.put(*unit);
        }
        self
    }

    /// Copy the contents of a flipped buffer into this (Java:
    /// CharBuffer.allocate(n).put(dest)).
    pub fn put_flipped(&mut self, other: &CharBuffer) -> &mut CharBuffer {
        self.put_units(&other.data[..other.limit]);
        self
    }

    /// The JavaString of the units between position and limit (Java:
    /// CharBuffer.toString() of the flipped buffer).
    pub fn to_java_string(&self) -> JavaString {
        JavaString::from_units(self.data[..self.limit].to_vec())
    }

    /// Advance the position by the consumed unit count.
    pub fn advance(&mut self, count: usize) {
        assert!(
            self.position + count <= self.limit,
            "BufferUnderflowException"
        );
        self.position += count;
    }

    /// The units between position and limit.
    pub fn remaining_units(&self) -> &[u16] {
        &self.data[self.position..self.limit]
    }

    /// The units between index 0 and limit.
    pub fn slice_upto_limit(&self) -> &[u16] {
        &self.data[..self.limit]
    }

    /// The units written since the last clear() (index 0..position), i.e. the
    /// translated output before flip().
    pub fn written_units(&self) -> &[u16] {
        &self.data[..self.position]
    }
}
