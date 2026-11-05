// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: the minimal BaseFileManager surface injected into PathFileObject.
// Why not 1:1: PathFileObject holds the upstream BaseFileManager instance; in Rust the
// PathFileObject is created by a file manager that is still being ported (JavacFileManager
// S3c), so the two operations PathFileObject performs on the manager are expressed
// as this interface. The 1:1 BaseFileManager struct (base_file_manager.rs, S3a) implements
// it; the remaining upstream methods live on that struct.

//! Minimal BaseFileManager interface for PathFileObject.

use crate::file::path_file_object::PathFileObject;
use javac_fs::Path as FsPath;

/// Minimal interface of the file manager used by [PathFileObject].
///
/// the 1:1 com.sun.tools.javac.file.BaseFileManager struct implements this
/// interface; it exists so PathFileObject can hold the manager through Rc<dyn ...> without
/// a self-referential Rc cycle (registered adapter, section 9).
pub trait BaseFileManagerApi {
    /// Java: BaseFileManager.updateLastUsedTime().
    fn update_last_used_time(&self);

    /// Java: BaseFileManager.flushCache(JavaFileObject).
    fn flush_cache(&self, file: &PathFileObject);

    /// Read a system-modules image resource for a JRTFileObject.
    ///
    /// Adapter: the upstream JRTFileObject reads the jrt: filesystem through
    /// java.nio.file; the port reads the system modules image through the
    /// JRTIndex (registered adapter).
    fn jrt_read_resource(&self, path: &FsPath) -> Option<Vec<u8>>;

    /// Read a jar/zip entry through the manager's per-archive reader.
    ///
    /// Adapter: the upstream PathFileObject reads through the FileSystem held
    /// by JavacFileManager.ArchiveContainer (JavacFileManager.java:557-575,
    /// PathFileObject.java:459); this lets JavacFileManager reuse one opened
    /// archive per path instead of reopening it for every entry. The default
    /// returns None so other managers keep their existing read path.
    fn open_jar_entry(
        &self,
        _archive_path: &FsPath,
        _entry_name: &str,
    ) -> Option<Vec<u8>> {
        None
    }

    /// Java: BaseFileManager.decode(ByteBuffer, boolean).
    fn decode(&self, inbuf: &[u8], ignore_encoding_errors: bool) -> javac_strings::JavaString;

    /// Java: BaseFileManager.getDecoder(String, boolean).
    fn get_decoder(
        &self,
        encoding_name: &javac_strings::JavaString,
        ignore: bool,
    ) -> Result<javac_charset::CharsetDecoder, crate::util::jc_diagnostic::Error>;

    /// Java: BaseFileManager.getEncodingName().
    fn get_encoding_name(&self) -> javac_strings::JavaString;

    /// Java: BaseFileManager.makeByteBuffer(InputStream).
    fn make_byte_buffer(
        &self,
        input: &mut dyn std::io::Read,
    ) -> Result<Vec<u8>, javac_tools::error::Error>;

    /// Java: BaseFileManager.recycleByteBuffer(ByteBuffer).
    fn recycle_byte_buffer(&self, buffer: Vec<u8>);

    /// Java: BaseFileManager.getCachedContent(JavaFileObject).
    fn get_cached_content(&self, file: &PathFileObject) -> Option<javac_strings::JavaString>;

    /// Java: BaseFileManager.cache(JavaFileObject, CharBuffer).
    fn cache(&self, file: &PathFileObject, cb: javac_strings::JavaString);

    /// Java: fileManager.log.useSource(this) (PathFileObject.getCharContent);
    /// returns the previous source to restore it later.
    fn use_source(
        &self,
        file: &PathFileObject,
    ) -> Option<std::rc::Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>;

    /// Java: fileManager.log.useSource(prev).
    fn restore_source(
        &self,
        previous: Option<
            std::rc::Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>,
        >,
    );
}
