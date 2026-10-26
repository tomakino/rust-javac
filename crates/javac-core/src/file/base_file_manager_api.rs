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

/// Minimal interface of the file manager used by [PathFileObject].
///
/// PORTING_RULES: the 1:1 com.sun.tools.javac.file.BaseFileManager struct implements this
/// interface; it exists so PathFileObject can hold the manager through Rc<dyn ...> without
/// a self-referential Rc cycle (registered adapter, section 9).
pub trait BaseFileManagerApi {
    /// Java: BaseFileManager.updateLastUsedTime().
    fn update_last_used_time(&self);

    /// Java: BaseFileManager.flushCache(JavaFileObject).
    fn flush_cache(&self, file: &PathFileObject);
}
