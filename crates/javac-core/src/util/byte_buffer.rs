// Origin: com/sun/tools/javac/util/ByteBuffer.java @ 982d53c (jdk-17.0.19-ga)

//! A byte buffer is a flexible array which grows when elements are
//! appended. There are also methods to append names to byte buffers
//! and to convert byte buffers to names.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: Java byte[] is Vec<u8>. The two public fields stay public because javac reads
//! them directly (PoolWriter: out.write(pool.poolbuf.elems, 0, pool.poolbuf.length)).
//! PORTING_RULES: appendLong/appendFloat/appendDouble and getLong/getFloat/getDouble use the
//! Java DataOutputStream/DataInputStream big-endian byte order; Rust uses the equivalent
//! to_be_bytes/from_be_bytes conversions of the same bit patterns.
//! PORTING_RULES: appendStream takes std::io::Read instead of java.io.InputStream.
//! InputStream.available() only pre-sizes the array and has no std::io::Read equivalent, so no
//! pre-allocation is performed (it cannot change the bytes read); the Java finally-close is the
//! caller's responsibility in Rust. The upstream quirk that appendStream does not update the
//! length field is ported verbatim (both call sites read through explicit offsets and never use
//! length after appendStream).

use std::io::{self, Read};

use super::array_utils;
use super::name::Name;
use super::names::Names;

/// A byte buffer is a flexible array which grows when elements are appended.
pub struct ByteBuffer {
    /// An array holding the bytes in this buffer; can be grown.
    ///
    /// Java: public byte[] elems;
    pub elems: Vec<u8>,

    /// The current number of defined bytes in this buffer.
    ///
    /// Java: public int length;
    pub length: i32,
}

impl ByteBuffer {
    /// Create a new byte buffer.
    ///
    /// Java: public ByteBuffer() { this(64); }
    pub fn new() -> ByteBuffer {
        ByteBuffer::with_initial_size(64)
    }

    /// Create a new byte buffer with an initial elements array
    /// of given size.
    ///
    /// Java: public ByteBuffer(int initialSize)
    pub fn with_initial_size(initial_size: i32) -> ByteBuffer {
        ByteBuffer {
            elems: vec![0u8; initial_size as usize],
            length: 0,
        }
    }

    /// Append byte to this buffer.
    ///
    /// Java: public void appendByte(int b)
    pub fn append_byte(&mut self, b: i32) {
        // Java: elems = ArrayUtils.ensureCapacity(elems, length);
        self.elems = array_utils::ensure_capacity_bytes(std::mem::take(&mut self.elems), self.length);
        // Java: elems[length++] = (byte)b;
        self.elems[self.length as usize] = b as u8;
        self.length += 1;
    }

    /// Append 'len' bytes from byte array,
    /// starting at given 'start' offset.
    ///
    /// Java: public void appendBytes(byte[] bs, int start, int len)
    pub fn append_bytes_from(&mut self, bs: &[u8], start: i32, len: i32) {
        // Java: elems = ArrayUtils.ensureCapacity(elems, length + len);
        self.elems =
            array_utils::ensure_capacity_bytes(std::mem::take(&mut self.elems), self.length + len);
        // Java: System.arraycopy(bs, start, elems, length, len);
        self.elems[self.length as usize..(self.length + len) as usize]
            .copy_from_slice(&bs[start as usize..(start + len) as usize]);
        self.length += len;
    }

    /// Append all bytes from given byte array.
    ///
    /// Java: public void appendBytes(byte[] bs)
    pub fn append_bytes(&mut self, bs: &[u8]) {
        let len = bs.len() as i32;
        self.append_bytes_from(bs, 0, len);
    }

    /// Append a character as a two byte number.
    ///
    /// Java: public void appendChar(int x)
    pub fn append_char(&mut self, x: i32) {
        // Java: elems = ArrayUtils.ensureCapacity(elems, length + 1);
        self.elems =
            array_utils::ensure_capacity_bytes(std::mem::take(&mut self.elems), self.length + 1);
        let len = self.length as usize;
        // Java: elems[length  ] = (byte)((x >>  8) & 0xFF);
        self.elems[len] = ((x >> 8) & 0xFF) as u8;
        // Java: elems[length+1] = (byte)((x      ) & 0xFF);
        self.elems[len + 1] = (x & 0xFF) as u8;
        self.length += 2;
    }

    /// Append an integer as a four byte number.
    ///
    /// Java: public void appendInt(int x)
    pub fn append_int(&mut self, x: i32) {
        // Java: elems = ArrayUtils.ensureCapacity(elems, length + 3);
        self.elems =
            array_utils::ensure_capacity_bytes(std::mem::take(&mut self.elems), self.length + 3);
        let len = self.length as usize;
        // Java: elems[length  ] = (byte)((x >> 24) & 0xFF);
        self.elems[len] = ((x >> 24) & 0xFF) as u8;
        // Java: elems[length+1] = (byte)((x >> 16) & 0xFF);
        self.elems[len + 1] = ((x >> 16) & 0xFF) as u8;
        // Java: elems[length+2] = (byte)((x >>  8) & 0xFF);
        self.elems[len + 2] = ((x >> 8) & 0xFF) as u8;
        // Java: elems[length+3] = (byte)((x      ) & 0xFF);
        self.elems[len + 3] = (x & 0xFF) as u8;
        self.length += 4;
    }

    /// Append a long as an eight byte number.
    ///
    /// Java: public void appendLong(long x) — DataOutputStream.writeLong (big-endian)
    pub fn append_long(&mut self, x: i64) {
        let bytes = x.to_be_bytes();
        self.append_bytes(&bytes);
    }

    /// Append a float as a four byte number.
    ///
    /// Java: public void appendFloat(float x) — DataOutputStream.writeFloat (big-endian)
    pub fn append_float(&mut self, x: f32) {
        let bytes = x.to_bits().to_be_bytes();
        self.append_bytes(&bytes);
    }

    /// Append a double as a eight byte number.
    ///
    /// Java: public void appendDouble(double x) — DataOutputStream.writeDouble (big-endian)
    pub fn append_double(&mut self, x: f64) {
        let bytes = x.to_bits().to_be_bytes();
        self.append_bytes(&bytes);
    }

    /// Append a name.
    ///
    /// Java: public void appendName(Name name) {
    ///     appendBytes(name.getByteArray(), name.getByteOffset(), name.getByteLength());
    /// }
    pub fn append_name(&mut self, name: &Name) {
        self.append_bytes_from(
            name.get_byte_array(),
            name.get_byte_offset(),
            name.get_byte_length(),
        );
    }

    /// Append the content of a given input stream.
    ///
    /// Java: public void appendStream(InputStream is) throws IOException
    pub fn append_stream<R: Read>(&mut self, is: &mut R) -> io::Result<()> {
        // Java: int start = length;
        let start = self.length;
        // Java: int initialSize = is.available();
        // PORTING_RULES: no equivalent on std::io::Read; only pre-sizing, see the file comment.
        let initial_size: i32 = 0;
        self.elems = array_utils::ensure_capacity_bytes(
            std::mem::take(&mut self.elems),
            self.length + initial_size,
        );
        let mut bp = start;
        loop {
            // Java: elems = ArrayUtils.ensureCapacity(elems, bp);
            self.elems =
                array_utils::ensure_capacity_bytes(std::mem::take(&mut self.elems), bp);
            // Java: r = is.read(elems, bp, elems.length - bp);
            let n = is.read(&mut self.elems[bp as usize..])?;
            // std::io::Read reports end-of-stream as 0, java.io.InputStream as -1.
            if n == 0 {
                break;
            }
            bp += n as i32;
        }
        // Java: the loop above does not assign length; ported verbatim.
        Ok(())
    }

    /// Extract an integer at position bp from elems.
    ///
    /// Java: public int getInt(int bp)
    pub fn get_int(&self, bp: i32) -> i32 {
        let bp = bp as usize;
        // Java:
        //   ((elems[bp] & 0xFF) << 24) + ((elems[bp+1] & 0xFF) << 16) +
        //   ((elems[bp+2] & 0xFF) << 8) + (elems[bp+3] & 0xFF)
        // The left shift of a large byte value wraps as in Java; Rust's << on i32 silently drops
        // the high bits, which is the same bit pattern.
        ((self.elems[bp] as i32 & 0xFF) << 24)
            + ((self.elems[bp + 1] as i32 & 0xFF) << 16)
            + ((self.elems[bp + 2] as i32 & 0xFF) << 8)
            + (self.elems[bp + 3] as i32 & 0xFF)
    }

    /// Extract a long integer at position bp from elems.
    ///
    /// Java: public long getLong(int bp)
    pub fn get_long(&self, bp: i32) -> i64 {
        let bp = bp as usize;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.elems[bp..bp + 8]);
        i64::from_be_bytes(bytes)
    }

    /// Extract a float at position bp from elems.
    ///
    /// Java: public float getFloat(int bp)
    pub fn get_float(&self, bp: i32) -> f32 {
        let bp = bp as usize;
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.elems[bp..bp + 4]);
        f32::from_bits(u32::from_be_bytes(bytes))
    }

    /// Extract a double at position bp from elems.
    ///
    /// Java: public double getDouble(int bp)
    pub fn get_double(&self, bp: i32) -> f64 {
        let bp = bp as usize;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.elems[bp..bp + 8]);
        f64::from_bits(u64::from_be_bytes(bytes))
    }

    /// Extract a character at position bp from elems.
    ///
    /// Java: public char getChar(int bp)
    pub fn get_char(&self, bp: i32) -> u16 {
        let bp = bp as usize;
        // Java: (char)(((elems[bp] & 0xFF) << 8) + (elems[bp+1] & 0xFF))
        ((self.elems[bp] as u16 & 0xFF) << 8) + (self.elems[bp + 1] as u16 & 0xFF)
    }

    /// Java: public byte getByte(int bp)
    pub fn get_byte(&self, bp: i32) -> i8 {
        self.elems[bp as usize] as i8
    }

    /// Reset to zero length.
    ///
    /// Java: public void reset() { length = 0; }
    pub fn reset(&mut self) {
        self.length = 0;
    }

    /// Convert contents to name.
    ///
    /// Java: public Name toName(Names names) { return names.fromUtf(elems, 0, length); }
    pub fn to_name(&self, names: &Names) -> Name {
        names.from_utf_range(&self.elems, 0, self.length)
    }
}

impl Default for ByteBuffer {
    fn default() -> Self {
        ByteBuffer::new()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::util::java_string::JavaString;

    #[test]
    fn initial_state() {
        let buf = ByteBuffer::new();
        assert_eq!(buf.length, 0);
        assert_eq!(buf.elems.len(), 64);
        let small = ByteBuffer::with_initial_size(4);
        assert_eq!(small.elems.len(), 4);
    }

    #[test]
    fn append_byte_and_get_byte() {
        let mut buf = ByteBuffer::new();
        buf.append_byte(0x41);
        buf.append_byte(0x1FF);
        assert_eq!(buf.length, 2);
        assert_eq!(buf.get_byte(0), 0x41);
        assert_eq!(buf.get_byte(1), -1); // (byte)0x1FF == -1
    }

    #[test]
    fn append_int_and_get_int_big_endian() {
        let mut buf = ByteBuffer::new();
        buf.append_int(0x01020304);
        buf.append_int(-1);
        assert_eq!(buf.length, 8);
        assert_eq!(buf.get_int(0), 0x01020304);
        assert_eq!(buf.get_int(4), -1);
        assert_eq!(&buf.elems[0..4], &[1, 2, 3, 4]);
        assert_eq!(&buf.elems[4..8], &[0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn append_char_and_get_char() {
        let mut buf = ByteBuffer::new();
        buf.append_char('A' as i32);
        buf.append_char(0xFFFF);
        assert_eq!(buf.length, 4);
        assert_eq!(buf.get_char(0), 'A' as u16);
        assert_eq!(buf.get_char(2), 0xFFFF);
        assert_eq!(&buf.elems[0..2], &[0, 0x41]);
    }

    #[test]
    fn append_long_float_double_round_trip() {
        let mut buf = ByteBuffer::new();
        buf.append_long(0x0102030405060708);
        buf.append_long(-1);
        buf.append_float(3.5);
        buf.append_double(-2.25);
        assert_eq!(buf.get_long(0), 0x0102030405060708);
        assert_eq!(buf.get_long(8), -1);
        assert_eq!(buf.get_float(16), 3.5);
        assert_eq!(buf.get_double(20), -2.25);
        assert_eq!(&buf.elems[0..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn append_bytes_and_growth() {
        let mut buf = ByteBuffer::with_initial_size(2);
        buf.append_bytes(&[1, 2, 3]);
        assert!(buf.elems.len() >= 3);
        assert_eq!(buf.length, 3);
        buf.append_bytes_from(&[9, 8, 7, 6], 1, 2);
        assert_eq!(buf.length, 5);
        assert_eq!(&buf.elems[..5], &[1, 2, 3, 8, 7]);
        // growth keeps the buffer large enough for many appends
        for i in 0..200 {
            buf.append_byte(i);
        }
        assert_eq!(buf.length, 205);
        // (byte)199 == -57
        assert_eq!(buf.get_byte(204), 199u8 as i8);
    }

    #[test]
    fn reset_only_clears_length() {
        let mut buf = ByteBuffer::new();
        buf.append_int(7);
        buf.reset();
        assert_eq!(buf.length, 0);
        assert_eq!(buf.get_int(0), 7); // the underlying array is untouched
    }

    #[test]
    fn append_stream_reads_all_bytes() {
        let mut buf = ByteBuffer::new();
        let data = vec![1u8, 2, 3, 4, 5];
        let mut cursor = Cursor::new(data.clone());
        buf.append_stream(&mut cursor).unwrap();
        assert_eq!(&buf.elems[..5], &data[..]);
        // Java: appendStream does not update length (ported quirk)
        assert_eq!(buf.length, 0);
    }

    #[test]
    fn append_name_and_to_name() {
        let names = Names::new(false);
        let name = names.from_string(&JavaString::from("java/lang/Object"));
        let mut buf = ByteBuffer::new();
        buf.append_name(&name);
        assert_eq!(buf.length, name.get_byte_length());
        let round_trip = buf.to_name(&names);
        assert_eq!(round_trip, name);
    }
}
