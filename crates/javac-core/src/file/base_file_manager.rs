// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2 ClassReader / API.
// REGISTERED units (dependencies unavailable, no substitute logic):
// - run (BaseFileManager.java:148)
// - handleFileManagerOption (BaseFileManager.java:230)
// - getValue (BaseFileManager.java:484)
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
//! (S3a):
//! * Java abstract class (implements JavaFileManager) -> Rust struct holding the shared
//!   state; the JavaFileManager methods are provided by JavacFileManager (S3c), which
//!   delegates the two methods BaseFileManager implements (handleOption/isSupportedOption).
//!   The abstract/protected hook createLocations() stays a private constructor helper.
//! * java.nio.file.Path -> javac-fs Path; java.io.InputStream -> std::io::Read and
//!   java.nio.ByteBuffer -> Vec<u8> (registered shape difference: makeByteBuffer reads to
//!   EOF instead of using InputStream.available(), same observable bytes).
//! * java.nio.charset is the registered javac-charset adapter (whole-input decode/encode):
//!   getEncodingName/decode are ported; the streaming CharsetDecoder used by
//!   PathFileObject.openReader is REGISTERED as unavailable in the javac-charset
//!   adapter (whole-input decode/encode), as is Charset.defaultCharset() for
//!   getDefaultEncodingName.
//! * java.lang.Thread/wait (deferredClose) are ported through std::thread +
//!   std::sync::{Mutex, Condvar} (DeferredCloseState; the BaseFileManager monitor
//!   is the shared state and the deferred-close thread waits on the condition
//!   variable). java.net.URL/URLClassLoader and reflection (getClassLoader) and
//!   java.lang.ref.SoftReference (content cache) are unavailable: getClassLoader is
//!   REGISTERED as not declared (URL/URLClassLoader unavailable); the content cache keeps the decoded JavaString directly
//!   (SoftReference is a memory-management detail with no observable behaviour
//!   without GC pressure; registered).
//!   cloneable identity and Java uses identity equality; the path-based key is the
//!   registered approximation — content is validated by getLastModified, so only cache hit
//!   rates can differ, never decoded content).
//! * handleOption(String, Iterator) and handleOptions are implemented in
//!   handle_option (298-325), which calls Option.process/OptionHelper (ported).
//! * the MULTIRELEASE case of handleOption(Option, String) is ported 1:1 (Option.MULTIRELEASE
//!   was restored at the Lead's decision); Locations.setMultiReleaseValue is a plain field
//!   until the S3b handler chain stores it.
//! * setContext is implemented (166-172) and calls Locations.update (184) with
//!   the Log/Options/CacheFSInfo services.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Read;
use std::option::Option as StdOption;
use std::rc::Rc;

use javac_charset::{
    ByteBuffer, CharBuffer, Charset, CharsetDecoder, CodingErrorAction,
};
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
use crate::util::abstract_log::AbstractLog;
use crate::util::assert;
use crate::util::options::Options;

/// Utility methods for building a file manager.
pub struct BaseFileManager {
    /// Java: public Log log;
    ///
    /// Set by setContext/init_services (implemented).
    #[allow(dead_code)]
    pub log: StdOption<Rc<crate::util::log::Log>>,

    /// Java: protected Charset charset; (None = platform default, like a null charset).
    ///
    /// java.nio.charset is the registered javac-charset adapter.
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
    /// The shared deferred-close state (Java: the synchronized object
    /// monitor + lastUsedTime/deferredCloseTimeout of BaseFileManager; the port
    /// shares it with the deferred-close thread — registered adapter).
    deferred: Arc<DeferredCloseState>,

    /// Java: private String encodingName;
    #[allow(dead_code)]
    encoding_name: RefCell<StdOption<JavaString>>,

    /// Java: protected String multiReleaseValue;
    ///
    /// Set by the MULTIRELEASE branch of handle_option_value (Option.MULTIRELEASE restored).
    #[allow(dead_code)]
    multi_release_value: RefCell<StdOption<JavaString>>,

    /// Java: private final ByteBufferCache byteBufferCache.
    byte_buffer_cache: RefCell<ByteBufferCache>,

    /// Java: protected final Map<JavaFileObject, ContentCacheEntry> contentCache.
    content_cache: Arc<Mutex<HashMap<String, ContentCacheEntry>>>,
}

impl BaseFileManager {
    /// Java: protected BaseFileManager(Charset charset).
    ///
    /// the JDK home is injected (Java Locations reads the java.home system
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
            deferred: Arc::new(DeferredCloseState::new()),
            encoding_name: RefCell::new(None),
            multi_release_value: RefCell::new(None),
            byte_buffer_cache: RefCell::new(ByteBufferCache::new()),
            content_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Java: protected Locations createLocations() { return new Locations(); }
    ///
    /// the JDK home is injected into Locations (see the file header).
    #[allow(dead_code)]
    fn create_locations(java_home: FsPath) -> Locations {
        Locations::new(java_home)
    }

    /// Java: public void setContext(Context context).
    ///
    /// Calls init_services(log, options, CacheFSInfo::new()); the FSInfo Context
    /// factory is registered as unavailable (ADR 0001 explicit construction).
    pub fn set_context(&mut self, context: &mut crate::util::context::Context) {
        let log = crate::util::log::Log::instance(context);
        let options = crate::util::options::Options::instance(context);
        // Java: FSInfo.instance(context); REGISTERED: the FSInfo Context factory is
        // unavailable, so the same object class is constructed directly (ADR 0001).
        self.init_services(log, options, Rc::new(CacheFSInfo::new()));
    }

    /// Shared body of setContext for the injected-service constructor (JavacFileManager::new).
    pub(crate) fn init_services(
        &mut self,
        log: Rc<crate::util::log::Log>,
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
            self.deferred.set_timeout(millis);
        }
    }

    /// Java: protected final Locations locations; (the field is exposed to JavacFileManager).
    /// Wired to the file manager; read-only access.
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
        self.deferred.timeout_ms()
    }

    /// Java: protected final Map<JavaFileObject, ContentCacheEntry> contentCache; (clear()).
    /// Java: protected void clearContentCache() — contentCache.clear().
    pub(crate) fn clear_content_cache(&mut self) {
        self.content_cache
            .lock()
            .expect("content cache")
            .clear();
    }

    /// The shared deferred-close state (used by the file-manager close path).
    pub(crate) fn deferred_state(&self) -> Arc<DeferredCloseState> {
        Arc::clone(&self.deferred)
    }

    /// The shared content-cache handle (cleared by the deferred-close thread).
    pub(crate) fn content_cache_handle(
        &self,
    ) -> Arc<Mutex<HashMap<String, ContentCacheEntry>>> {
        Arc::clone(&self.content_cache)
    }

    /// Java: synchronized void updateLastUsedTime().
    /// Java: synchronized void updateLastUsedTime() (BaseFileManager.java:168-172):
    /// refreshes the last-used time when a deferred close is armed.
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: BaseFileManager.java:168-172.
    pub fn update_last_used_time(&self) {
        if self.deferred.timeout_ms() > 0 {
            self.last_used_time.set(now_millis());
            self.deferred.update_last_used();
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
    // REGISTERED: java.net.URL/URLClassLoader and the reflective custom loader
    // lookup are unavailable, so the entry point is not declared.
    // (not declared: getClassLoader)

    /// Java: public boolean isDefaultBootClassPath().
    pub fn is_default_boot_class_path(&mut self) -> bool {
        self.locations.is_default_boot_class_path()
    }

    /// Java: public boolean isDefaultSystemModulesPath().
    pub fn is_default_system_modules_path(&self) -> bool {
        self.locations.is_default_system_modules_path()
    }

    /// Java: public boolean handleOption(String current, Iterator<String> remaining)
    /// (BaseFileManager.java:190-247).
    ///
    /// the upstream uncaught IllegalArgumentException
    /// (Option.InvalidValueException wrapped in IllegalArgumentException) is
    /// returned as Err(message); the JavaFileManager wrapper panics with it.
    ///
    /// # Arguments
    ///   - current: the option string;
    ///   - remaining: the remaining command line arguments.
    ///
    /// # Returns
    ///   - Ok(false) when the option is not a file manager option;
    ///   - Ok(true) when it was handled; Err(IllegalArgumentException message).
    ///
    /// # Panics
    ///   - none.
    pub fn handle_option(
        &mut self,
        current: &str,
        remaining: &mut dyn Iterator<Item = JavaString>,
    ) -> Result<bool, JavaString> {
        // Java: OptionHelper helper = new GrumpyHelper(log) { get/put/remove ->
        // options; handleFileManagerOption -> handleOption(option, value) };
        let mut helper = BaseFileManagerOptionHelper {
            options: self.options.clone(),
            log: self.log.clone(),
            applied: Vec::new(),
        };
        let Some(option) =
            JavacOption::lookup_in(current, &JavacOption::get_javac_file_manager_options())
        else {
            return Ok(false);
        };
        let mut rest = remaining.map(|argument| argument.to_string_lossy());
        if let Err(error) = option.handle_option(&mut helper, current, &mut rest) {
            // Java: throw new IllegalArgumentException(e.getMessage(), e);
            return Err(JavaString::from(error.get_message()));
        }
        let applied = helper.applied;
        for (option, value) in applied {
            self.handle_option_value(option, &value);
        }
        Ok(true)
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
    /// the ENCODING branch is ported; every other option is forwarded to
    /// Locations.handleOption (S3b).  The MULTIRELEASE branch needs Option.MULTIRELEASE, which
    /// was restored (Option.MULTIRELEASE); the branch below sets multi_release_value.
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

    /// Java: public boolean handleOptions(Map<Option, String> map)
    /// (BaseFileManager.java:275-291).
    ///
    /// the upstream catch (IllegalArgumentException) reports
    /// Errors.IllegalArgumentForOption; the ported Locations.handleOption
    /// reports through its own path instead of throwing (registered deviation,
    /// see the ported Locations), so the catch branch has no throwing source.
    ///
    /// # Arguments
    ///   - map: the option/value entries.
    ///
    /// # Returns
    ///   - true if all the calls are successful.
    ///
    /// # Panics
    ///   - none.
    pub fn handle_options(&mut self, map: &[(JavacOption, JavaString)]) -> bool {
        let mut ok = true;
        for (option, value) in map {
            let value = value.to_string_lossy();
            let result = self.handle_option_value(*option, &value);
            ok = ok & result;
        }
        ok
    }

    /// Java: private String getDefaultEncodingName() {
    ///           if (defaultEncodingName == null)
    ///               defaultEncodingName = Charset.defaultCharset().name();
    ///           return defaultEncodingName; }
    fn get_default_encoding_name(&self) -> JavaString {
        Charset::default_charset_name()
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
    /// the streaming CharsetDecoder of upstream getDecoder has no Rust
    /// counterpart in the javac-charset adapter (whole-input decode/encode); only decode()
    /// below is expressed; the streaming decoder is REGISTERED as unavailable and
    /// PathFileObject.openReader is a separate registered unit.
    fn get_charset(&self, encoding_name: &JavaString) -> Result<Charset, crate::util::jc_diagnostic::Error> {
        match &self.charset {
            Some(charset) => Ok(*charset),
            None => match Charset::for_name(encoding_name) {
                Ok(charset) => Ok(charset),
                // Java: BaseFileManager.java decode catch
                // (IllegalCharsetNameException|UnsupportedCharsetException):
                //   log.error(Errors.UnsupportedEncoding(encName));
                //   return CharBuffer.allocate(1).flip();
                Err(_) => Err(crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::unsupported_encoding(
                        encoding_name.clone(),
                    ),
                )),
            },
        }
    }

    /// Java: public CharBuffer decode(ByteBuffer inbuf, boolean
    /// ignoreEncodingErrors) (BaseFileManager.java:417-470).
    ///
    /// ByteBuffer -> &[u8] wrapped into the ported ByteBuffer
    /// view; CharBuffer -> JavaString; the decoder is the incremental
    /// javac-charset CharsetDecoder (the java.nio.charset state machine).
    pub fn decode(&self, inbuf: &[u8], ignore_encoding_errors: bool) -> JavaString {
        let encoding_name = self.get_encoding_name();
        let mut decoder = match self.get_decoder(&encoding_name, ignore_encoding_errors) {
            Ok(decoder) => decoder,
            Err(error) => {
                if let Some(log) = &self.log {
                    log.error_pos(None, error);
                }
                // Java: return CharBuffer.allocate(1).flip() (the empty buffer).
                return JavaString::from("");
            }
        };
        // Java: slightly overestimate the buffer size to avoid reallocation.
        let factor = decoder.average_chars_per_byte() * 0.8 + decoder.max_chars_per_byte() * 0.2;
        let mut input = ByteBuffer::wrap(inbuf);
        let mut dest = CharBuffer::allocate(10 + (input.remaining() as f32 * factor) as usize);
        loop {
            let result = decoder.decode(&mut input, &mut dest, true);
            dest.flip();
            if result.is_underflow() {
                // Java: make sure there is at least one extra character.
                if dest.limit() == dest.capacity() {
                    let mut bigger = CharBuffer::allocate(dest.capacity() + 1);
                    bigger.put_flipped(&dest);
                    dest = bigger;
                }
                return dest.to_java_string();
            } else if result.is_overflow() {
                let capacity = 10
                    + dest.capacity()
                    + (input.remaining() as f32 * decoder.max_chars_per_byte()) as usize;
                let mut bigger = CharBuffer::allocate(capacity);
                bigger.put_flipped(&dest);
                dest = bigger;
            } else if result.is_malformed() || result.is_unmappable() {
                // Java: bad character in input — collect the bytes and report.
                let length = result.length();
                let mut unmappable = String::new();
                for _ in 0..length {
                    unmappable.push_str(&format!("{:02X}", input.get()));
                }
                let charset_name = self
                    .charset
                    .map(|charset| charset.name())
                    .unwrap_or_else(|| encoding_name.clone());
                if let Some(log) = &self.log {
                    log.error_at(
                        dest.limit() as i32,
                        crate::util::jc_diagnostic::Error::from(
                            crate::resources::compiler_properties::errors::illegal_char_for_encoding(
                                JavaString::from(unmappable.as_str()),
                                charset_name,
                            ),
                        ),
                    );
                }
                // Java: undo the flip() to prepare the output buffer for more
                // translation, then put U+FFFD.
                dest.set_position(dest.limit());
                dest.set_limit(dest.capacity());
                dest.put(0xFFFD);
            } else {
                panic!("AssertionError: unexpected CoderResult");
            }
        }
    }

    /// Java: public CharsetDecoder getDecoder(String encodingName, boolean
    /// ignoreEncodingErrors) (BaseFileManager.java:389-403).
    ///
    /// # Arguments
    ///   - encoding_name: the requested encoding name;
    ///   - ignore: true selects CodingErrorAction.REPLACE, false REPORT.
    ///
    /// # Returns
    ///   - the decoder, or the UnsupportedEncoding error (Java throws
    ///     IllegalArgumentException/UnsupportedCharsetException).
    ///
    /// # Panics
    ///   - none.
    pub fn get_decoder(
        &self,
        encoding_name: &JavaString,
        ignore: bool,
    ) -> Result<CharsetDecoder, crate::util::jc_diagnostic::Error> {
        let charset = self.get_charset(encoding_name)?;
        let action = if ignore {
            CodingErrorAction::Replace
        } else {
            CodingErrorAction::Report
        };
        let mut decoder = charset.new_decoder();
        decoder
            .on_malformed_input(action)
            .on_unmappable_character(action);
        Ok(decoder)
    }

    /// Java: public ByteBuffer makeByteBuffer(InputStream in) throws IOException.
    ///
    /// InputStream -> &mut dyn Read (reads to EOF instead of using
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
        let mut cache = self.content_cache.lock().expect("content cache");
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
            .lock()
            .expect("content cache")
            .insert(key, ContentCacheEntry::new(file, cb));
    }

    /// Java: public void flushCache(JavaFileObject file).
    pub fn flush_cache_object(&self, file: &PathFileObject) {
        let key = file.get_name().to_string_lossy();
        self.content_cache.lock().expect("content cache").remove(&key);
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
    /// Rust collections cannot contain a null element, so the element check is
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

    fn jrt_read_resource(&self, _path: &javac_fs::Path) -> Option<Vec<u8>> {
        None
    }

    fn decode(&self, inbuf: &[u8], ignore_encoding_errors: bool) -> JavaString {
        BaseFileManager::decode(self, inbuf, ignore_encoding_errors)
    }

    fn get_decoder(
        &self,
        encoding_name: &JavaString,
        ignore: bool,
    ) -> Result<CharsetDecoder, crate::util::jc_diagnostic::Error> {
        BaseFileManager::get_decoder(self, encoding_name, ignore)
    }

    fn get_encoding_name(&self) -> JavaString {
        BaseFileManager::get_encoding_name(self)
    }

    fn make_byte_buffer(
        &self,
        input: &mut dyn Read,
    ) -> Result<Vec<u8>, Error> {
        BaseFileManager::make_byte_buffer(self, input)
    }

    fn recycle_byte_buffer(&self, buffer: Vec<u8>) {
        BaseFileManager::recycle_byte_buffer(self, buffer);
    }

    fn get_cached_content(&self, file: &PathFileObject) -> Option<JavaString> {
        BaseFileManager::get_cached_content(self, file)
    }

    fn cache(&self, file: &PathFileObject, cb: JavaString) {
        BaseFileManager::cache(self, file, cb);
    }

    fn use_source(
        &self,
        file: &PathFileObject,
    ) -> StdOption<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>> {
        let log = self.log.as_ref()?;
        let source: Rc<dyn javac_tools::tools::java_file_object::JavaFileObject> =
            Rc::new(file.clone());
        use crate::util::abstract_log::AbstractLog;
        log.use_source(Some(source))
    }

    fn restore_source(
        &self,
        previous: StdOption<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>,
    ) {
        if let Some(log) = &self.log {
            use crate::util::abstract_log::AbstractLog;
            log.use_source(previous);
        }
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

/// The shared state of the deferred close (Java: synchronized BaseFileManager
/// monitor with lastUsedTime/deferredCloseTimeout; BaseFileManager.java:145-175).
/// The file manager and the deferred-close thread share it through Arc.
pub struct DeferredCloseState {
    /// The mutex-protected last-used time and timeout (Java: the monitor fields).
    inner: Mutex<DeferredCloseInner>,
    /// The condition variable replacing Object.wait (BaseFileManager.java:153).
    condvar: Condvar,
}

/// The mutable part of DeferredCloseState (Java: lastUsedTime/deferredCloseTimeout).
struct DeferredCloseInner {
    /// Java: private long lastUsedTime
    last_used: Instant,
    /// Java: protected long deferredCloseTimeout (millis; 0 means no deferral)
    timeout_ms: i64,
}

impl DeferredCloseState {
    /// Java: private long lastUsedTime = System.currentTimeMillis() together with
    /// the default deferredCloseTimeout = 0 (BaseFileManager.java:174-175).
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - the fresh state with the current instant as the last-used time.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: BaseFileManager.java:174-175.
    pub fn new() -> DeferredCloseState {
        DeferredCloseState {
            inner: Mutex::new(DeferredCloseInner {
                last_used: Instant::now(),
                timeout_ms: 0,
            }),
            condvar: Condvar::new(),
        }
    }

    /// Java: the setContext deferredCloseTimeout assignment (BaseFileManager.java:104-112).
    pub fn set_timeout(&self, millis: i64) {
        self.inner.lock().expect("deferred close state").timeout_ms = millis;
    }

    /// The current deferred-close timeout in milliseconds (0 = disabled).
    pub fn timeout_ms(&self) -> i64 {
        self.inner.lock().expect("deferred close state").timeout_ms
    }

    /// Java: synchronized void updateLastUsedTime() (BaseFileManager.java:168-172).
    pub fn update_last_used(&self) {
        let mut inner = self.inner.lock().expect("deferred close state");
        if inner.timeout_ms > 0 {
            inner.last_used = Instant::now();
        }
    }

    /// Java: protected void deferredClose() (BaseFileManager.java:145-166): spawn a
    /// daemon thread that waits for the configured inactivity period and then
    /// runs the close action.
    ///
    /// # Arguments
    ///   - close: the close action (the file-manager cleanup), which must be
    ///     Send because the Java thread receives the captured state.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: BaseFileManager.java:145-166.
    pub fn spawn_deferred_close<F>(self: &Arc<Self>, close: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let state = Arc::clone(self);
        std::thread::spawn(move || {
            loop {
                let wait_for = {
                    let inner = state.inner.lock().expect("deferred close state");
                    if inner.timeout_ms <= 0 {
                        None
                    } else {
                        let deadline =
                            inner.last_used + Duration::from_millis(inner.timeout_ms as u64);
                        let now = Instant::now();
                        if now >= deadline {
                            None
                        } else {
                            Some(deadline - now)
                        }
                    }
                };
                let Some(wait_for) = wait_for else {
                    break;
                };
                let guard = state.inner.lock().expect("deferred close state");
                let _ = state.condvar.wait_timeout(guard, wait_for);
            }
            state.inner.lock().expect("deferred close state").timeout_ms = 0;
            close();
        });
    }
}
pub(crate) struct ContentCacheEntry {
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
    use super::{BaseFileManager, DeferredCloseState};
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
    fn handle_option_encodes_and_stores_file_manager_values() {
        use crate::main::option::Option as JavacOption;
        let mut context = crate::util::context::Context::new();
        let options = crate::util::options::Options::instance(&mut context);
        let log = crate::util::log::Log::instance(&mut context);
        let mut manager =
            BaseFileManager::new(None, FsPath::from_path_buf(std::path::PathBuf::from(".")));
        manager.init_services(log, options, std::rc::Rc::new(
            crate::file::cache_fs_info::CacheFSInfo::new(),
        ));

        // Java: BaseFileManager.handleOption(Option.ENCODING, value).
        let mut rest = vec![js("UTF-16LE")].into_iter();
        assert!(manager.handle_option("-encoding", &mut rest).expect("handled"));
        assert_eq!(manager.get_encoding_name().to_string_lossy(), "UTF-16LE");

        // An unknown option is not handled.
        assert!(!manager
            .handle_option("--definitely-not-an-option", &mut std::iter::empty())
            .expect("not handled"));

        // Java: handleOptions(Map<Option,String>): every entry is applied.
        assert!(manager.handle_options(&[(JavacOption::ENCODING, js("UTF-8"))]));
        assert_eq!(manager.get_encoding_name().to_string_lossy(), "UTF-8");
    }

    #[test]
    fn handle_option_misses_are_not_errors() {
        let mut manager =
            BaseFileManager::new(None, FsPath::from_path_buf(std::path::PathBuf::from(".")));
        assert!(!manager
            .handle_option("--not-an-option", &mut std::iter::empty())
            .expect("not handled"));
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

    /// Java: BaseFileManager.updateLastUsedTime (BaseFileManager.java:168-172):
    /// the last-used time is refreshed only while a deferred close is armed.
    #[test]
    fn deferred_close_state_refreshes_the_last_used_time_only_when_armed() {
        let state = DeferredCloseState::new();
        state.set_timeout(100);
        let before = state.inner.lock().expect("state").last_used;
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.update_last_used();
        let after = state.inner.lock().expect("state").last_used;
        assert!(after > before, "the last-used time is refreshed");
        state.set_timeout(0);
        let before_disabled = state.inner.lock().expect("state").last_used;
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.update_last_used();
        assert_eq!(
            state.inner.lock().expect("state").last_used,
            before_disabled,
            "no refresh without an armed timeout"
        );
    }
}

/// Java: the anonymous OptionHelper created by BaseFileManager.handleOption
/// (BaseFileManager.java:203-234): the value accessors delegate to the
/// Options table and the file manager options are recorded for
/// handleOption(Option, String).
struct BaseFileManagerOptionHelper {
    options: StdOption<Rc<Options>>,
    log: StdOption<Rc<crate::util::log::Log>>,
    applied: Vec<(JavacOption, String)>,
}

impl crate::main::option_helper::OptionHelper for BaseFileManagerOptionHelper {
    fn get(&self, option: JavacOption) -> StdOption<String> {
        self.options.as_ref().and_then(|options| options.get_option(option))
    }

    fn put(&mut self, name: &str, value: &str) {
        if let Some(options) = &self.options {
            options.put(name, value);
        }
    }

    fn remove(&mut self, name: &str) {
        if let Some(options) = &self.options {
            options.remove(name);
        }
    }

    fn handle_file_manager_option(&mut self, option: JavacOption, value: &str) -> bool {
        self.applied.push((option, value.to_string()));
        true
    }

    fn get_log(&self) -> StdOption<Rc<crate::util::log::Log>> {
        self.log.clone()
    }

    fn get_own_name(&self) -> String {
        panic!("IllegalStateException")
    }

    fn add_file(&mut self, _path: &javac_fs::Path) {
        panic!("IllegalArgumentException")
    }

    fn add_class_name(&mut self, _name: &str) {
        panic!("IllegalArgumentException")
    }
}
