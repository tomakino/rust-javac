// Origin: com/sun/tools/javac/file/BaseFileManager.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2 ClassReader / API.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: run (BaseFileManager.java:148)
// - NOT PORTED: handleFileManagerOption (BaseFileManager.java:230)
// - NOT PORTED: getValue (BaseFileManager.java:484)
// End of Gap registration.

//! Utility methods for building a file manager.
//! There are no references here to file-system specific objects such as
//! java.io.File or java.nio.file.Path.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (S3a):
//! * Java abstract class (implements JavaFileManager) -> Rust struct holding the shared
//!   state; the JavaFileManager methods are provided by JavacFileManager (S3c), which
//!   delegates the two methods BaseFileManager implements (handleOption/isSupportedOption).
//!   The abstract/protected hook createLocations() stays a private constructor helper.
//! * java.nio.file.Path -> javac-fs Path; java.io.InputStream -> std::io::Read and
//!   java.nio.ByteBuffer -> Vec<u8> (registered shape difference: makeByteBuffer reads to
//!   EOF instead of using InputStream.available(), same observable bytes).
//! * java.nio.charset is the registered javac-charset adapter (whole-input decode/encode):
//!   getEncodingName/decode are ported; the streaming CharsetDecoder used by
//!   PathFileObject.openReader and the unsupported-charset/malformed-input Log reporting are
//!   TODO (the Log error entry point and a streaming decoder are not available).
//!   getDefaultEncodingName needs Charset.defaultCharset(), which the adapter does not
//!   provide yet (TODO, reported).
//! * java.lang.Thread/wait (deferredClose), java.net.URL/URLClassLoader and reflection
//!   (getClassLoader) and java.lang.ref.SoftReference (content cache) are not ported:
//!   deferredClose/getClassLoader are unimplemented + TODO; the content cache keeps the
//!   decoded JavaString directly (SoftReference is a memory-management detail with no
//!   observable behaviour without GC pressure; registered).
//! * contentCache is keyed by the file object's Java name (PathFileObject does not expose a
//!   cloneable identity and Java uses identity equality; the path-based key is the
//!   registered approximation — content is validated by getLastModified, so only cache hit
//!   rates can differ, never decoded content).
//! * TODO(main/OptionHelper): handleOption(String, Iterator) needs Option.process and
//!   main.OptionHelper (not ported); handleOptions needs Log.error and the Errors
//!   resource for IllegalArgumentForOption (Log has no error entry point in this).
//! * the MULTIRELEASE case of handleOption(Option, String) is ported 1:1 (Option.MULTIRELEASE
//!   was restored at the Lead's decision); Locations.setMultiReleaseValue is a plain field
//!   until the S3b handler chain stores it.
//! * setContext awaits Locations.update (S3b) and therefore stays unimplemented for now.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Read;
use std::option::Option as StdOption;
use std::rc::Rc;

use javac_charset::{Charset, MalformedAction};
use javac_fs::Path as FsPath;
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::tools::file_object::FileObject;
use javac_tools::tools::java_file_object::Kind;

use crate::file::base_file_manager_api::BaseFileManagerApi;
use crate::file::cache_fs_info::CacheFSInfo;
use crate::file::locations::Locations;
use crate::file::path_file_object::PathFileObject;
use crate::main::option::Option as JavacOption;
use crate::util::assert;
use crate::util::options::Options;

/// Utility methods for building a file manager.
pub struct BaseFileManager {
    /// Java: public Log log;
    ///
    /// TODO(S3b): set by setContext once Locations.update is ported.
    #[allow(dead_code)]
    pub log: StdOption<Rc<RefCell<crate::util::log::Log>>>,

    /// Java: protected Charset charset; (None = platform default, like a null charset).
    ///
    /// PORTING_RULES: java.nio.charset is the registered javac-charset adapter.
    charset: StdOption<Charset>,

    /// Java: protected Options options;
    #[allow(dead_code)]
    options: StdOption<Rc<Options>>,

    /// Java: protected String classLoaderClass;
    #[allow(dead_code)]
    class_loader_class: StdOption<JavaString>,

    /// Java: protected final Locations locations;
    #[allow(dead_code)]
    locations: Locations,

    /// Java: public boolean autoClose;
    pub auto_close: bool,

    /// Java: synchronized void updateLastUsedTime() / private long lastUsedTime.
    last_used_time: Cell<i64>,

    /// Java: protected long deferredCloseTimeout = 0;
    deferred_close_timeout: Cell<i64>,

    /// Java: private String encodingName;
    #[allow(dead_code)]
    encoding_name: RefCell<StdOption<JavaString>>,

    /// Java: protected String multiReleaseValue;
    ///
    /// TODO(main option restore): set by the MULTIRELEASE branch of handleOption(Option,String).
    #[allow(dead_code)]
    multi_release_value: RefCell<StdOption<JavaString>>,

    /// Java: private final ByteBufferCache byteBufferCache.
    byte_buffer_cache: RefCell<ByteBufferCache>,

    /// Java: protected final Map<JavaFileObject, ContentCacheEntry> contentCache.
    content_cache: RefCell<HashMap<String, ContentCacheEntry>>,
}

impl BaseFileManager {
    /// Java: protected BaseFileManager(Charset charset).
    ///
    /// PORTING_RULES: the JDK home is injected (Java Locations reads the java.home system
    /// property); S3c constructs this through JavacFileManager.
    #[allow(dead_code)]
    pub(crate) fn new(charset: StdOption<Charset>, java_home: FsPath) -> BaseFileManager {
        BaseFileManager {
            log: None,
            charset,
            options: None,
            class_loader_class: None,
            locations: BaseFileManager::create_locations(java_home),
            auto_close: false,
            last_used_time: Cell::new(now_millis()),
            deferred_close_timeout: Cell::new(0),
            encoding_name: RefCell::new(None),
            multi_release_value: RefCell::new(None),
            byte_buffer_cache: RefCell::new(ByteBufferCache::new()),
            content_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Java: protected Locations createLocations() { return new Locations(); }
    ///
    /// PORTING_RULES: the JDK home is injected into Locations (see the file header).
    #[allow(dead_code)]
    fn create_locations(java_home: FsPath) -> Locations {
        Locations::new(java_home)
    }

    /// Java: public void setContext(Context context).
    ///
    /// TODO(S3b): the body needs Locations.update(log, warn, FSInfo.instance(context)) and
    /// the FSInfo Context registration (not ported); no substitute state is set.
    pub fn set_context(&mut self, context: &mut crate::util::context::Context) {
        let log = crate::util::log::Log::instance(context);
        let options = crate::util::options::Options::instance(context);
        // Java: FSInfo.instance(context); the FSInfo Context factory is not ported, so the
        // same object class is created directly (registered deviation, no substitute data).
        self.init_services(log, options, Rc::new(CacheFSInfo::new()));
    }

    /// Shared body of setContext for the injected-service constructor (JavacFileManager::new).
    pub(crate) fn init_services(
        &mut self,
        log: Rc<std::cell::RefCell<crate::util::log::Log>>,
        options: Rc<Options>,
        fs_info: Rc<CacheFSInfo>,
    ) {
        self.log = Some(Rc::clone(&log));
        self.options = Some(Rc::clone(&options));
        let warn = options.is_lint_set("path");
        self.locations.update(log, warn, fs_info);
        // Java: options.get("fileManager.deferClose"): seconds as float; invalid -> 60s.
        if let Some(value) = options.get("fileManager.deferClose") {
            let millis = match value.parse::<f32>() {
                Ok(seconds) => (seconds * 1000.0) as i64,
                Err(_) => 60 * 1000,
            };
            self.deferred_close_timeout.set(millis);
        }
    }

    /// Java: protected final Locations locations; (the field is exposed to JavacFileManager).
    /// Not wired yet (TODO(file manager wiring)).
    #[allow(dead_code)]
    pub(crate) fn locations(&self) -> &Locations {
        &self.locations
    }

    /// Java: protected final Locations locations; (mutable access for the file manager).
    pub(crate) fn locations_mut(&mut self) -> &mut Locations {
        &mut self.locations
    }

    /// Java: protected long deferredCloseTimeout; (read by JavacFileManager.close()).
    pub(crate) fn deferred_close_timeout(&self) -> i64 {
        self.deferred_close_timeout.get()
    }

    /// Java: protected final Map<JavaFileObject, ContentCacheEntry> contentCache; (clear()).
    pub(crate) fn clear_content_cache(&mut self) {
        self.content_cache.borrow_mut().clear();
    }

    /// Java: synchronized void updateLastUsedTime().
    pub fn update_last_used_time(&self) {
        if self.deferred_close_timeout.get() > 0 {
            self.last_used_time.set(now_millis());
        }
    }

    /// Java: public void clear() { new HashSet<>(options.keySet()).forEach(k -> options.remove(k)); }
    pub fn clear(&mut self) {
        let options = self
            .options
            .as_ref()
            .expect("NullPointerException: options");
        for key in options.key_set() {
            options.remove(&key);
        }
    }

    // Java: protected ClassLoader getClassLoader(URL[] urls).
    //
    // TODO(P6): java.net.URL/URLClassLoader and the reflective custom loader lookup are not
    // ported; the entry point is not declared while the URL type is unavailable.
    // (not declared: getClassLoader)

    /// Java: public boolean isDefaultBootClassPath().
    pub fn is_default_boot_class_path(&mut self) -> bool {
        self.locations.is_default_boot_class_path()
    }

    /// Java: public boolean isDefaultSystemModulesPath().
    pub fn is_default_system_modules_path(&self) -> bool {
        self.locations.is_default_system_modules_path()
    }

    /// Java: public boolean handleOption(String current, Iterator<String> remaining).
    ///
    /// TODO(main/OptionHelper): needs Option.process/handleOption and main.OptionHelper.
    pub fn handle_option(
        &self,
        _current: &str,
        _remaining: &mut dyn Iterator<Item = JavaString>,
    ) -> bool {
        unimplemented!(
            "TODO(main/OptionHelper batch): BaseFileManager.handleOption needs Option.process"
        )
    }

    /// Java: public int isSupportedOption(String option).
    pub fn is_supported_option(&self, option: &str) -> i32 {
        let option = JavacOption::lookup_in(option, &JavacOption::get_javac_file_manager_options());
        match option {
            None => -1,
            Some(option) => {
                if option.has_arg() {
                    1
                } else {
                    0
                }
            }
        }
    }

    /// Java: public boolean handleOption(Option option, String value).
    ///
    /// PORTING_RULES: the ENCODING branch is ported; every other option is forwarded to
    /// Locations.handleOption (S3b).  The MULTIRELEASE branch needs Option.MULTIRELEASE, which
    /// was trimmed in task-48 (reported; TODO(main option restore)).
    pub fn handle_option_value(&mut self, option: JavacOption, value: &str) -> bool {
        match option {
            JavacOption::ENCODING => {
                *self.encoding_name.borrow_mut() = Some(JavaString::from(value));
                true
            }
            JavacOption::MULTIRELEASE => {
                *self.multi_release_value.borrow_mut() = Some(JavaString::from(value));
                self.locations.set_multi_release_value(value);
                true
            }
            _ => self.locations_mut().handle_option(option, Some(value)),
        }
    }

    /// Java: public boolean handleOptions(Map<Option, String> map).
    ///
    /// TODO(Log): needs log.error(Errors.IllegalArgumentForOption(...)) — the Log error
    /// entry point is not ported in this; no substitute reporting is written.
    pub fn handle_options(&self, _map: &[(JavacOption, JavaString)]) -> bool {
        unimplemented!(
            "TODO(Log batch): BaseFileManager.handleOptions needs Log.error and Errors.IllegalArgumentForOption"
        )
    }

    /// Java: private String getDefaultEncodingName() { return Charset.defaultCharset().name(); }
    ///
    /// TODO(charset): the javac-charset adapter does not implement
    /// Charset.defaultCharset() yet (the JVM default charset has no Rust counterpart without
    /// the launcher's file.encoding handling); reported.
    #[allow(dead_code)]
    fn get_default_encoding_name(&self) -> JavaString {
        unimplemented!("TODO(charset batch): Charset.defaultCharset() is not ported")
    }

    /// Java: public String getEncodingName() {
    ///           return (encodingName != null) ? encodingName : getDefaultEncodingName(); }
    pub fn get_encoding_name(&self) -> JavaString {
        match self.encoding_name.borrow().as_ref() {
            Some(encoding_name) => encoding_name.clone(),
            None => self.get_default_encoding_name(),
        }
    }

    /// Java: Charset cs = (this.charset == null) ? Charset.forName(encodingName) : this.charset;
    ///
    /// PORTING_RULES: the streaming CharsetDecoder of upstream getDecoder has no Rust
    /// counterpart in the javac-charset adapter (whole-input decode/encode); only decode()
    /// below is expressed (PathFileObject.openReader is a separate TODO).
    fn get_charset(&self, encoding_name: &JavaString) -> Charset {
        match &self.charset {
            Some(charset) => *charset,
            None => match Charset::for_name(encoding_name) {
                Ok(charset) => charset,
                // Java: catch (IllegalCharsetNameException|UnsupportedCharsetException e)
                // { log.error(Errors.UnsupportedEncoding(encName)); return empty CharBuffer; }
                Err(_) => unimplemented!(
                    "TODO(Log batch): BaseFileManager.decode unsupported charset reporting needs log.error(Errors.UnsupportedEncoding)"
                ),
            },
        }
    }

    /// Java: public CharBuffer decode(ByteBuffer inbuf, boolean ignoreEncodingErrors).
    ///
    /// PORTING_RULES: ByteBuffer -> &[u8], CharBuffer -> JavaString (whole-input decode of the
    /// javac-charset adapter).  The malformed/unmappable branch of the upstream loop reports
    /// Errors.IllegalCharForEncoding and substitutes U+FFFD; that reporting needs the Log error
    /// entry point (not ported) and is a TODO panic.
    pub fn decode(&self, inbuf: &[u8], ignore_encoding_errors: bool) -> JavaString {
        let encoding_name = self.get_encoding_name();
        let charset = self.get_charset(&encoding_name);
        let action = if ignore_encoding_errors {
            MalformedAction::Replace
        } else {
            MalformedAction::Report
        };
        match charset.decode(inbuf, action) {
            Ok(text) => text,
            Err(_) => unimplemented!(
                "TODO(Log batch): BaseFileManager.decode malformed input reporting needs log.error(Errors.IllegalCharForEncoding)"
            ),
        }
    }

    // Java: public CharsetDecoder getDecoder(String, boolean).
    //
    // TODO(charset): the streaming CharsetDecoder has no Rust form in the javac-charset
    // adapter; decode() above covers the whole-input conversion (not declared: getDecoder).

    /// Java: public ByteBuffer makeByteBuffer(InputStream in) throws IOException.
    ///
    /// PORTING_RULES: InputStream -> &mut dyn Read (reads to EOF instead of using
    /// available()); ByteBuffer -> Vec<u8> (same observable bytes).
    pub fn make_byte_buffer(&self, input: &mut dyn Read) -> Result<Vec<u8>, Error> {
        let mut result = self.byte_buffer_cache.borrow_mut().get(1024);
        input
            .read_to_end(&mut result)
            .map_err(|error| Error::io(JavaString::from(error.to_string().as_str())))?;
        Ok(result)
    }

    /// Java: public void recycleByteBuffer(ByteBuffer bb).
    pub fn recycle_byte_buffer(&self, buffer: Vec<u8>) {
        self.byte_buffer_cache.borrow_mut().put(buffer);
    }

    /// Java: public CharBuffer getCachedContent(JavaFileObject file).
    pub fn get_cached_content(&self, file: &PathFileObject) -> StdOption<JavaString> {
        let key = file.get_name().to_string_lossy();
        let mut cache = self.content_cache.borrow_mut();
        match cache.get(&key) {
            None => None,
            Some(entry) => {
                if entry.is_valid(file) {
                    Some(entry.value.clone())
                } else {
                    cache.remove(&key);
                    None
                }
            }
        }
    }

    /// Java: public void cache(JavaFileObject file, CharBuffer cb).
    pub fn cache(&self, file: &PathFileObject, cb: JavaString) {
        let key = file.get_name().to_string_lossy();
        self.content_cache
            .borrow_mut()
            .insert(key, ContentCacheEntry::new(file, cb));
    }

    /// Java: public void flushCache(JavaFileObject file).
    pub fn flush_cache_object(&self, file: &PathFileObject) {
        let key = file.get_name().to_string_lossy();
        self.content_cache.borrow_mut().remove(&key);
    }

    /// Java: public static Kind getKind(Path path).
    pub fn get_kind(path: &FsPath) -> Kind {
        match path.file_name() {
            Some(name) => BaseFileManager::get_kind_name(&name.to_java_string()),
            None => Kind::OTHER,
        }
    }

    /// Java: public static Kind getKind(String name).
    pub fn get_kind_name(name: &str) -> Kind {
        if name.ends_with(Kind::CLASS.extension().to_string_lossy().as_str()) {
            Kind::CLASS
        } else if name.ends_with(Kind::SOURCE.extension().to_string_lossy().as_str()) {
            Kind::SOURCE
        } else if name.ends_with(Kind::HTML.extension().to_string_lossy().as_str()) {
            Kind::HTML
        } else {
            Kind::OTHER
        }
    }

    /// Java: protected static <T> T nullCheck(T o) { return Objects.requireNonNull(o); }
    pub fn null_check<T>(o: StdOption<T>) -> T {
        assert::check_non_null(o)
    }

    /// Java: protected static <T> Collection<T> nullCheck(Collection<T> it).
    ///
    /// PORTING_RULES: Rust collections cannot contain a null element, so the element check is
    /// guaranteed by the type system; the method keeps the upstream shape (registered).
    pub fn null_check_collection<T>(items: Vec<T>) -> Vec<T> {
        items
    }
}

impl BaseFileManagerApi for BaseFileManager {
    fn update_last_used_time(&self) {
        BaseFileManager::update_last_used_time(self);
    }

    fn flush_cache(&self, file: &PathFileObject) {
        BaseFileManager::flush_cache_object(self, file);
    }
}

/// Java: private static class ByteBufferCache.
struct ByteBufferCache {
    cached: StdOption<Vec<u8>>,
}

impl ByteBufferCache {
    #[allow(dead_code)]
    fn new() -> ByteBufferCache {
        ByteBufferCache { cached: None }
    }

    /// Java: ByteBuffer get(int capacity).
    fn get(&mut self, capacity: usize) -> Vec<u8> {
        let capacity = capacity.max(20_480);
        match self.cached.take() {
            Some(mut buffer) if buffer.capacity() >= capacity => {
                buffer.clear();
                buffer
            }
            _ => Vec::with_capacity(capacity + (capacity >> 1)),
        }
    }

    /// Java: void put(ByteBuffer x).
    fn put(&mut self, buffer: Vec<u8>) {
        self.cached = Some(buffer);
    }
}

/// Java: protected static class ContentCacheEntry.
struct ContentCacheEntry {
    /// Java: final long timestamp;
    timestamp: i64,
    /// Java: final SoftReference<CharBuffer> ref; — the decoded content is kept directly
    /// (SoftReference is a memory-management detail; registered).
    value: JavaString,
}

impl ContentCacheEntry {
    /// Java: ContentCacheEntry(JavaFileObject file, CharBuffer cb).
    fn new(file: &PathFileObject, cb: JavaString) -> ContentCacheEntry {
        ContentCacheEntry {
            timestamp: file.get_last_modified(),
            value: cb,
        }
    }

    /// Java: boolean isValid(JavaFileObject file).
    fn is_valid(&self, file: &PathFileObject) -> bool {
        self.timestamp == file.get_last_modified()
    }
}

/// Java: System.currentTimeMillis().
fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(error) => -(error.duration().as_millis() as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::BaseFileManager;
    use crate::file::path_file_object::PathFileObject;
    use javac_fs::Path as FsPath;
    use javac_strings::JavaString;
    use javac_tools::tools::java_file_object::Kind;
    use std::io::Cursor;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "javac-bfm-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ))
    }

    fn fs_path(path: &std::path::Path) -> FsPath {
        FsPath::from_path_buf(path.to_path_buf())
    }

    #[test]
    fn get_kind_matches_the_extension() {
        assert_eq!(BaseFileManager::get_kind_name("Hello.java"), Kind::SOURCE);
        assert_eq!(BaseFileManager::get_kind_name("Hello.class"), Kind::CLASS);
        assert_eq!(BaseFileManager::get_kind_name("index.html"), Kind::HTML);
        assert_eq!(BaseFileManager::get_kind_name("README"), Kind::OTHER);
        let path = fs_path(&std::path::PathBuf::from("some/dir/X.class"));
        assert_eq!(BaseFileManager::get_kind(&path), Kind::CLASS);
    }

    #[test]
    fn make_byte_buffer_reads_all_bytes_and_recycles() {
        let manager =
            BaseFileManager::new(None, FsPath::from_path_buf(std::path::PathBuf::from(".")));
        let mut input = Cursor::new(b"abcdef".to_vec());
        let buffer = manager.make_byte_buffer(&mut input).expect("buffer");
        assert_eq!(buffer, b"abcdef");
        manager.recycle_byte_buffer(buffer);
        let mut input = Cursor::new(b"xyz".to_vec());
        let buffer = manager.make_byte_buffer(&mut input).expect("buffer");
        assert_eq!(buffer, b"xyz");
    }

    #[test]
    fn content_cache_is_validated_by_last_modified() {
        let dir = temp_dir("cache");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("A.java");
        std::fs::write(&file, b"class A {}").expect("write");
        let path = fs_path(&file);
        let manager = Rc::new(BaseFileManager::new(
            None,
            FsPath::from_path_buf(std::path::PathBuf::from(".")),
        ));
        let manager_api: Rc<dyn crate::file::base_file_manager_api::BaseFileManagerApi> =
            manager.clone();
        let fo = PathFileObject::for_simple_path(manager_api, &path, &path);

        assert_eq!(manager.get_cached_content(&fo), None);
        manager.cache(&fo, js("class A {}"));
        assert_eq!(manager.get_cached_content(&fo), Some(js("class A {}")));
        manager.flush_cache_object(&fo);
        assert_eq!(manager.get_cached_content(&fo), None);

        // A changed last-modified time invalidates the cached entry.
        manager.cache(&fo, js("old"));
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&file, b"class A { int x; }").expect("rewrite");
        assert_eq!(manager.get_cached_content(&fo), None);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn is_supported_option_uses_the_file_manager_options() {
        let manager =
            BaseFileManager::new(None, FsPath::from_path_buf(std::path::PathBuf::from(".")));
        assert_eq!(manager.is_supported_option("-encoding"), 1);
        assert_eq!(manager.is_supported_option("--class-path"), 1);
        assert_eq!(manager.is_supported_option("--source-path"), 1);
        assert_eq!(manager.is_supported_option("-d"), 1);
        assert_eq!(manager.is_supported_option("--system"), 1);
        // -g is a BASIC option, not a file manager option.
        assert_eq!(manager.is_supported_option("-g"), -1);
        assert_eq!(manager.is_supported_option("--no-such-option"), -1);
    }

    #[test]
    fn handle_option_value_sets_the_encoding_name() {
        let mut manager =
            BaseFileManager::new(None, FsPath::from_path_buf(std::path::PathBuf::from(".")));
        assert!(manager.handle_option_value(crate::main::option::Option::ENCODING, "UTF-8"));
        assert_eq!(manager.encoding_name.borrow().clone(), Some(js("UTF-8")));
    }

    #[test]
    fn update_last_used_time_is_a_noop_without_deferred_close() {
        let manager =
            BaseFileManager::new(None, FsPath::from_path_buf(std::path::PathBuf::from(".")));
        let before = manager.last_used_time.get();
        manager.update_last_used_time();
        assert_eq!(manager.last_used_time.get(), before);
    }

    #[test]
    fn null_check_and_null_check_collection() {
        assert_eq!(BaseFileManager::null_check(Some(3u32)), 3);
        assert_eq!(
            BaseFileManager::null_check_collection(vec![1, 2]),
            vec![1, 2]
        );
        let result = std::panic::catch_unwind(|| BaseFileManager::null_check::<u32>(None));
        assert!(result.is_err());
    }
}
