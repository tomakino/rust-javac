//! DEFLATE decompression (raw deflate, no zlib wrapper).

use crate::error::Error;

/// Inflates a raw DEFLATE stream into a buffer of `size` bytes.
pub(crate) fn inflate(data: &[u8], size: usize, name: &str) -> Result<Vec<u8>, Error> {
    let mut output = vec![0u8; size];
    let mut inflate = zlib_rs::Inflate::new(false, 15);
    let status = inflate
        .decompress(data, &mut output, zlib_rs::InflateFlush::Finish)
        .map_err(|err| Error::Corrupted(format!("deflate error in {}: {err:?}", name)))?;
    if status != zlib_rs::Status::StreamEnd {
        return Err(Error::Corrupted(format!(
            "deflate error in {}: {status:?}",
            name
        )));
    }
    let produced = inflate.total_out() as usize;
    output.truncate(produced);
    Ok(output)
}
