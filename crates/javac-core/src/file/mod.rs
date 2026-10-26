//! com.sun.tools.javac.file
//!
//! The file layer of the compiler (Java: package com.sun.tools.javac.file).
//!
//! PORTING_RULES / status:
//! * ported earlier: RelativePath (relative_path.rs), PathFileObject
//!   (path_file_object.rs), FSInfo (fs_info.rs), CacheFSInfo (cache_fs_info.rs) and the
//!   minimal file-manager interface injected into PathFileObject
//!   (base_file_manager_api.rs, adapter);
//! * ported in the S3a of task-57: BaseFileManager (base_file_manager.rs; the
//!   charset/URL/thread-dependent methods are TODO and reported) and the Locations
//!   skeleton (locations.rs; the full LocationHandler chain is S3b);
//! * not ported yet (reported to the Lead): Locations handlers (S3b), JavacFileManager
//!   (S3c), JRTIndex (S3a, same as BaseFileManager) and the java.nio.file /
//!   java.util.jar / java.net URL / java.nio.charset dependencies those need;
//! * TODO(P2): the deferred methods listed in the individual file headers.

pub mod base_file_manager;
pub mod base_file_manager_api;
pub mod cache_fs_info;
pub mod fs_info;
pub mod javac_file_manager;
pub mod jrt_index;
pub mod locations;
pub mod path_file_object;
pub mod relative_path;
