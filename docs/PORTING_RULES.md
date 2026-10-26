# 移植规则(PORTING_RULES)

本文件是所有 Rust 代码与 OpenJDK javac 17 源码之间映射的唯一约定。任何新增约定必须先更新本
文件,再写代码。基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)。

## 1. 文件映射

- 一个 Java 源文件对应一个 Rust 源文件,路径按包镜像:

      Java:  src/jdk.compiler/share/classes/com/sun/tools/javac/util/Context.java
      Rust:  crates/javac-core/src/util/context.rs

- 文件名:类名 PascalCase 转 snake_case(Context.java -> context.rs)。
- 每个 Rust 文件第一行必须写来源锚点(不写版权头,由项目负责人统一添加):

      // Origin: com/sun/tools/javac/util/Context.java @ 982d53c (jdk-17.0.19-ga)

- 不允许把多个 Java 文件合并为一个 Rust 文件,也不允许一个 Java 文件拆成多个 Rust 文件。
  例外:登记在 §9 的适配文件。
- javac 包根文件(com/sun/tools/javac/Main.java 等)映射到 crates/javac-core/src/root/x.rs;
  package-info.java 裁剪(仅文档注释),不入 manifest。
- resources 目录的 properties 不是 Java 源文件:由 xtask gen-resources 生成
  crates/javac-core/src/resources/generated_*.rs(登记适配,§9)。
- Java 包目录与 Rust 模块一一对应(api/code/comp/file/jvm/launcher/main/model/parser/
  platform/processing/tree/util),其中 launcher 与 processing 已裁剪,不写实现。

## 2. 注释

- 保留上游英文 Javadoc 注释原文,放在对应的 Rust item 上(文件级或 /// 文档注释)。
  不翻译、不删减语义;便于升级时与上游 diff 对照。
- 与上游行为有关的补充说明以“// PORTING_RULES:”开头;TODO 必须写成
  “TODO(<阶段>): ...”,并同步登记 docs/STATUS.md。

## 3. 命名

- 类/接口/枚举 -> struct/enum/trait,名字保持 PascalCase 一致。
- 方法 -> snake_case,语义保持;静态方法用关联函数或自由函数;实例方法用 impl 方法。
- 字段 -> snake_case,顺序与上游保持一致。
- 常量 -> UPPER_SNAKE_CASE。
- Java 方法重载在 Rust 中用不同函数名表达,并在文件内用“// PORTING_RULES: overloads”
  列出映射,例如:

      ensureCapacity(byte[]) -> ensure_capacity_bytes
      ensureCapacity(char[]) -> ensure_capacity_u16
      ensureCapacity(int[])  -> ensure_capacity_i32
      ensureCapacity(T[])    -> ensure_capacity_refs

- 与 Rust 关键字冲突的标识符加下划线后缀(type -> type_, ref -> ref_)。
- Java enum 常量名保持原名(如 SourceVersion.RELEASE_17),在 enum 上加
  #[allow(non_camel_case_types)];需要与 Java 名称逐字对照的数据结构(如 Names 标准名)
  可保留 Java 字段名并在类型上加 #[allow(non_snake_case)]。
- 内部类/匿名类:嵌套 struct/enum,放对应模块内;需要外部状态时显式传 ctx 参数。

## 4. 对象表示与借用

- 共享可变对象图用 arena + ID:SymbolArena/SymbolId、TypeArena/TypeId、TreeArena/TreeId、
  ScopeArena/ScopeId。ID 类型在对应包首次需要时引入。完整契约见 docs/ADR/0001-arena-ids.md。
- 核心 crate 禁止 Rc<RefCell<...>> 表达对象图(CI 检查);组件状态放 CompilerContext
  的具名字段,方法通过显式 &mut ctx 传递。
- 不可变共享数据可用 Rc(例如不可变链表节点);凡影响可观察行为的共享/顺序语义必须与
  javac 一致。
- Java 引用数组(元素可为 null)-> Vec<Option<T>>。

## 5. 集合

- javac.util.List 用自实现不可变链表移植(共享 tail、O(1) prepend、算法照抄),不得换成
  Vec 替代语义。
- ListBuffer 照抄其增量构建语义。
- Java HashMap 在无迭代顺序依赖处用 std::collections::HashMap;凡迭代顺序可能影响输出
  (诊断、字节码、符号遍历)处必须使用保持顺序的实现,并在代码注释标明。
- Java LinkedHashMap/LinkedHashSet 需要保持插入顺序;Java TreeMap/TreeSet 需要保持排序
  (比较器语义照抄)。
- List 的 Java 引用比较(==/!=)必须用 ptr_eq,不得用 PartialEq;内容比较才用 PartialEq。
- ListBuffer 必须照抄 Java 的 prev/last 增量结构(set_tail 仅对未共享节点,Rc::get_mut 失败即
  panic);禁止 Vec + 惰性快照等替代实现。
- List.map 的"未变化返回 this"引用语义在 Rust 中无法跨类型判断,恒返回新列表(登记偏离)。
- Java 泛型擦除导致的 convert(Class,List) 强转不移植,由 Rust 类型系统承担。
- java.util.List 接口中无调用者的 mutator 方法省略,须在文件注释与 STATUS 登记。

## 6. 异常与错误

- CompletionFailure 等可捕获/重试的失败 -> Result<T, CompileError>(类型随 P2/P3 引入)。
- Abort/FatalError/ClientCodeException/PropagatedException -> panic payload(util 中已定义),
  仅在编译入口与 FFI 边界 catch_unwind。
- Assert.check 等 -> crate::util::assert。
- 禁止用 Option 静默吞掉错误,禁止改变异常路径的捕获/重试语义。

## 7. 字符串与字符

- Java String/char 语义统一用 crate::util::java_string::JavaString(UTF-16 code unit 序列)。
- 位置/长度/charAt/标识符判断/行号列号全部按 UTF-16 code unit 计算,与 Java 一致。
- 输出层(诊断/日志/文件写入)按平台 charset 编码,规则在 P6 输出层实现。
- Rust 的 char 表示码点,与 Java char 不同;不得直接混用。

## 8. 数值语义

- Java byte/short/char/int/long/float/double -> i8/i16/u16/i32/i64/f32/f64;
  boolean -> bool。
- Java 整数溢出回绕:用 wrapping_add/wrapping_mul 等显式表达;移位距离按 Java 规则
  (低 5/6 位)用 wrapping_shl/wrapping_shr,不得依赖 Rust 默认行为。
- 除法/取模在负数上与 Java 一致(截断向零),但除零行为必须按 Java 语义处理。
- 浮点运算、NaN 比较、常量折叠必须与 Java 表达式提升规则一致,照抄上游表达式;
  不得用 Rust 惯用捷径替代。

## 9. 适配文件白名单(允许非 1:1)

新增适配文件必须在 docs/STATUS.md 登记,并说明对应 Java 语义来源:

- util/java_string.rs:Java String/char 的 UTF-16 表示。
- util/name.rs(JCTree 前的名称表):NameTableHandle 以 Rc<RefCell<NameTableInner>> 实现
  interning(端口白名单:这是全项目唯一允许的 Rc<RefCell> 用法,借用窗口仅限表内插入);
  Name 句柄在 intern 时复制字节快照(Rc<[u8]>),等价于 Java 中对 table 字节的视图。
- util/names.rs:NamesStd/Names 拆分(Java 用 GC 回收 Name.Table.names 环,Rust 用 Weak);
  Names 通过 Deref 暴露标准名,字段名保持 Java 原名以便对照;构造顺序与 Java 构造器一致
  (Name 索引经 compareTo 可观察)。
- util/convert.rs:重载映射规则见文件头注释;utf2chars/chars2utf 系列拆为 _into/_range 变体。
- util/position.rs:LineMapImpl/LineTabMapImpl 两个内部类合并为 enum 变体(字段/算法/方法一一对应)。
- util/iterators.rs:Java Iterator/Iterable 映射为 std::iter::Iterator;NoSuchElementException 用 panic 对齐。
- util/graph_utils.rs:DottableNode/DotVisitor/toDot 裁剪(依赖 identity hash 与 Hashtable 迭代顺序,
  无法逐字对齐;-X 调试输出),Tarjan 核心照抄;登记 SUPPORT_MATRIX。
- crate javac-strings:JavaString(UTF-16)底层共享 crate;javac-tools 与 javac-core 都依赖它,
  javac-core 的 util/java_string.rs 仅做 re-export。
- crates/javac-core/src/resources/:xtask 从上游 properties 生成的诊断资源表(不手改)。
- crate xtask:开发工具(资源生成、manifest),不属于移植产物。
- crates/javac-tools/src/{error,uri,java_io,jdk}.rs:javax.tools 所需的 JDK 类型最小适配
  (IOException/URI/Reader/Writer/Class/ClassLoader/ServiceLoader 等),每个文件头注明对应
  Java 类型与原因。
- (后续)util/arena.rs 或等价:arena/ID 基础设施。
- (后续)util/compiler_context.rs:CompilerContext 服务/状态容器,对应 javac Context 的用法。
- (后续)错误类型定义。
- crate javac-tools:javax.tools 接口的 trait 化(对应 Java 接口,算 1:1 接口映射,但语言形态不同)。

## 10. 验证

- 每个移植文件必须带单测:覆盖上游算法边界与 Java 语义差异点。
- 提交前必须 cargo test 通过;阶段性任务完成后由 Lead 跑 cargo fmt/clippy/test。
- 静默降级禁止:未实现路径必须 unimplemented!/panic,并在 STATUS 中登记;不得返回假数据。

## 11. 禁止事项

- 禁止简化实现、改写算法、合并阶段、调整遍历顺序。
- 禁止用“看起来等价”的 Rust 惯用法替代会影响输出的 Java 行为。
- 禁止引入未登记依赖;第三方依赖必须经 Lead 审核并在 docs/DEPENDENCIES.md 登记。
- 禁止自创抽象层或 helper 层(适配文件白名单除外)。
- 禁止在移植文件中写版权头(由项目负责人统一处理)。

## 12. 接口层(javax.tools / javax.lang.model)契约

- Java 接口 -> trait(可 dyn);共享身份用 Rc<dyn Trait>;CompilationTask 返回 Box(Java GC
  引用 -> Rust 独占所有权)。唯一允许的 Rc<RefCell<dyn Writer>> 在 JavaCompiler.get_task 的
  out 参数(Java 可变 Writer 引用)。
- DiagnosticCollector.getDiagnostics 返回 Vec 快照(Java 返回不可变视图)。
- StandardLocation.locationFor 缓存用 thread_local(JDK 为跨线程 ConcurrentMap;单线程编译器
  语义等价,identity 契约保持)。
- JavaFileManager.Location.isModuleOrientedLocation:上游用 getName().matches(模块词边界正则),
  Matcher.matches 要求整体匹配,实际等价于 name == "MODULE";照抄并在代码注释登记怪癖。
- File/Path 统一 std::path::PathBuf;File 与 Path 两组方法保留不同函数名。
- Reader/Writer -> java_io 最小字符流 trait;InputStream/OutputStream -> std::io::Read/Write。
- 异常映射:checked IOException -> Result<T, Error>;UnsupportedOperationException -> Err;
  其它 RuntimeException(IllegalArgumentException/IllegalStateException/NullPointerException 等)
  -> panic(与 Java 未捕获行为一致)。
- 适配文件:crates/javac-tools/src/{error,uri,java_io,jdk}.rs;URI 为 java.net.URI 最小映射
  (normalize/resolve/relativize/toURL 等在 file 包移植时按需补齐,先登记 TODO)。
- Class/ClassLoader/ServiceLoader/Processor 为占位类型(APT 裁剪);
  SimpleJavaFileObject 的 Java 子类化 -> Rust 组合 + 重实现虚调用点。
- SourceVersion.latestSupported 固定 RELEASE_17(基线 JDK 17)。
- javax.lang.model 的 Java 集合参数/返回 -> Vec<X>;Name/CharSequence 接口方法用 JavaString
  语义(javac-core 的 util::Name 后续通过 adapter 实现接口层 Name trait)。

## 13. 接口层对象向下转换(ADR 0001 配套)

- Rust 的 Any downcast 只能用于具体类型,不能 downcast 到 trait object;Java
  `instanceof` + 强转在接口层用显式方法表达:
  Element::as_package_element/as_type_element/as_variable_element/as_executable_element/
  as_type_parameter_element/as_record_component_element/as_module_element,Directive::
  as_exports_directive/as_opens_directive/as_provides_directive/as_requires_directive/
  as_uses_directive;默认返回 None,具体实现类型 override。
- ElementFilter 等按 Java 实现顺序使用这些方法;方法命名 as_<java 接口 snake_case>。
- 核心(P3 Symbol adapter)实现这些 trait 时必须 override 对应方法。

## 14. com.sun.source 与 javax.lang.model 映射契约补充

- Java 接口 -> trait;Java 抽象类(如 AbstractElementVisitor6/TreeScanner/SimpleTreeVisitor/
  JavacTask/Trees)-> trait(默认实现)或 trait + 具体结构,按 Java 层次 1:1 对应。
- visitor 接口方法在 Rust 中以 panic 默认实现占位(Java 编译期强制实现 -> Rust 运行期 panic),
  具体 util 类层次 trait 覆盖;登记为语言形态差异。
- Java 泛型双分派(Tree.accept(visitor, p) / Element.accept / TypeMirror.accept)在 Rust 中
  由 util 层 dispatch 函数复现(按 getKind + as_xxx 转换);对象向下转换契约见 §13。
- Java 抽象类在子类重声明为 abstract 的方法,在 Rust 子 trait 中不强制实现(不改变可观察行为)。
- Java 单参数便利重载(仅传 null 参数)不移植;集合映射:List<? extends X> -> Vec<Rc<dyn X>>,
  Map -> Vec<(K,V)>,Set 重载 -> *_set 后缀 + Vec(保持输入顺序);ElementFilter 返回借用引用。
- AnnotationValue 的 Object 值 -> 封闭 enum Value/ValueRef;未知值形状不可出现。
- Unknown*Exception/Mirrored*Exception 不移植序列化与 Object 参数;消息用 Display(Java toString)。
- Java assert -> debug_assert;switch default 的 AssertionError -> panic(消息照抄)。
- 仅测试使用的 fixture 文件(如 util/test_support.rs)作为适配文件登记。
- Tree.Kind 枚举实际 **115 个常量**(含 preview:GUARDED_PATTERN/PARENTHESIZED_PATTERN/
  DEFAULT_CASE_LABEL 等),已按 Java 声明顺序完整移植;javac-core 的 TypeTag.getKindLiteral 依赖它
  (目标数量为 115)。
- Tree 的 as_xxx 向下转换访问器(74 个)遵循 §13,命名 as_<java 接口 snake>(去掉 Tree 后缀);
  具体实现(P1 JCTree)必须按 kind override,否则 util dispatch panic。
- Tree.Kind::as_interface() -> Option<&'static str>(Java 对 OTHER 返回 null -> None;不依赖反射)。
- LiteralTree::get_value() -> Option<Value>(Java Object 的封闭表示,复用 javax.lang.model 的
  Value:primitive/String/null 变体)。
- com.sun.source 类型映射:Tree 引用 -> Rc<dyn X>;List<? extends X> -> Vec<Rc<dyn X>>;
  Set<Modifier> -> Vec<Modifier>;String/CharSequence -> JavaString;可空返回按 Javadoc 中
  "null" 的启发式 -> Option(已人工核对 CaseTree/CompilationUnitTree/NewArrayTree;P1 实现时
  发现误判再修正);Scope 的 enclosing scope/class/method -> Option。
- javac-source / javac-tools 的 Cargo.toml 设 [lib] doctest=false(Java 示例注释不能当 Rust
  doctest 跑),属可接受的适配。

### 14.1 com.sun.source.doctree / util 补充契约(第 2 批)

- DocTree.Kind 无 asInterface,但有 tag_name() -> Option<JavaString>(Java final String tagName,
  无参构造为 null)。
- DocTree 的 as_xxx 访问器遵循 §13 树版本(37/38 个)。
- AttributeTree.ValueKind 为节点文件内嵌套 enum。
- DocTree 节点接口中的 default 方法(getFullBody/getPreamble/getPostamble/isInline 等)按 trait
  默认实现;DocTreeVisitor 的 7 个 default visit* 转发 visitOther。
- util 层:TreePath/DocTreePath 具体类 -> struct(不可变链 parent+leaf);TreeScanner/
  SimpleTreeVisitor/DocTreeScanner/SimpleDocTreeVisitor -> trait(§14 模式);JavacTask/Trees/
  DocTrees 抽象类 -> trait(实现留 P6);dispatch_tree 由 Kind + as_xxx 复现(Tree.accept 泛型
  仅 where Self: Sized)。

### 14.2 com.sun.source.doctree 契约

- DocTree.Kind 39 常量(Java 顺序);tag_name() -> Option<JavaString>(Java final String tagName,
  无 tag 的构造器为 null);25 个有 tag,14 个为 None。
- DocTree 的 34 个 as_xxx 对象转换访问器(§13 树版本;实现按 Kind override)。
- AttributeTree.ValueKind 为 attribute_tree 模块内嵌套 enum(EMPTY/UNQUOTED/SINGLE/DOUBLE)。
- DocTreeVisitor 36 个方法:7 个 Java default(visitDocType/Hidden/Index/Provides/Summary/
  SystemProperty/Uses)在 Rust 中为 trait 默认并转发 visit_other;其余 29 个 panic 默认。
- DocCommentTree 的 get_full_body/get_preamble/get_postamble 为 trait 默认实现
  (first_sentence++body;空 Vec);ReturnTree.is_inline 默认 false。
- ErroneousTree.get_diagnostic -> Option<Rc<dyn Diagnostic<JavaFileObjectRef>>>(可空启发式;
  Java 文档注明可 null)。

### 14.3 com.sun.source.util 契约(接口与小文件批次)

- JavacTask 为 trait,父 trait 为 javac_tools::tools::java_compiler::CompilationTask(对应 Java
  implements CompilationTask);实现留 P6。JavacTask.instance(ProcessingEnvironment) 不移植
  (依赖未移植的 javax.annotation.processing/BasicJavacTask/Context),TODO(P6)。
- JavacTask.getElements()/getTypes() 暂不声明:返回类型 javax.lang.model.util.Elements/Types
  按此前裁决推迟到 P6;P6 必须补齐(登记为 P6 依赖项)。
- TaskEvent:Java final class -> struct;构造器重载 -> 不同名构造器;toString -> Display
  (null 字段按 Java 拼接 "null")。
- SourcePositions/DocSourcePositions 的 Java 重载 -> Rust 子 trait 重声明同名方法,调用需完全
  限定(§14 语言形态限制)。
- TreePath:具体类 -> struct(不可变链 compilation_unit + parent + leaf);CompilationUnitTree
  字段用 Rc<dyn Tree> + §13 as_compilation_unit_tree 转换;Iterable<Tree> -> iterator() 返回
  Vec;Iterator.remove 不移植。get_path/get_path_in_path 依赖 TreePathScanner,在 TreeScanner
  批次回填(当前为 TODO(P1) unimplemented,必须跟踪到回填完成)。
- TaskListener/Plugin 的 Java default 方法为 Rust 默认方法(TaskListener 空实现;
  Plugin.auto_start=false);Plugin.init 的 JavacTask 参数用 Rc<RefCell<dyn JavacTask>>
  (init 会注册监听器,共享可变任务)。

### 14.4 TreePathScanner / SimpleTreeVisitor 决策(task-13 裁决)

- TreePathScanner 采用"独立 trait"方案(不继承 TreeScanner):Java TreePathScanner 通过重写
  scan(Tree,P) 维护 currentPath,而 Rust trait 默认方法中的 self.scan 会静态解析到 TreeScanner,
  无法动态分派。为使路径语义与 javac 一致:
  * TreePathScanner trait 的 supertrait 为 TreeVisitor<Option<R>,P>,自带 66 个 visit 默认方法
    (由与 TreeScanner 相同的脚本从 Java 源码机械生成;子节点 scan 全部走本 trait 的 scan);
  * required 方法 get_current_path()/set_current_path()(trait 无字段,由实现者提供状态);
  * scan(tree: Rc<dyn Tree>, p) 默认实现照抄 Java:保存 prev,currentPath = TreePath::new_child
    (prev, tree),r = super 扫描,currentPath = prev,返回 r;另提供 scan_root/scan_opt;
  * 形态差异登记:不继承 TreeScanner、实现者需提供 current_path 状态。
- TreePath.get_path/get_path_in_path 在 TreePathScanner 落地后按 Java TreePath 源码回填,
  移除 should_panic("TODO(P1)") 标记。
- SimpleTreeVisitor:default_value() -> Option<&R>(Java DEFAULT_VALUE 可 null);
  default_action(&mut self, tree: &dyn Tree, p: P) -> R(Java defaultAction(Object,P) 的
  等价表达;default_value 为 None 时 panic);第三份 dispatch_simple_tree_visitor(115 Kind
  到 visit 方法,返回 R)。
- Trees/DocTrees 抽象类 -> trait(实现 P6);Doc 家族按 §14.2/§14.3 处理。

### 14.5 TreePathScanner 落地与 dispatch 家族

- TreePathScanner 按 §14.4 方案 B 落地:独立 trait(不继承 TreeScanner),supertrait
  TreeVisitor<Option<R>,P>;required get_current_path/set_current_path;scan 的树参数由
  &dyn Tree 改为 Rc<dyn Tree>(TreePath 需要所有权),Java null 树由 scan_opt(Option<Rc<dyn Tree>>)
  表达;prev=None 时仅允许 CompilationUnit root(对应 Java NPE);scan_path 照抄 scan(TreePath,P)。
- dispatch 家族(适配文件)已有四份:dispatch_tree(TreeVisitor)、dispatch_tree_scanner、
  dispatch_tree_path_scanner、dispatch_simple_tree_visitor(待实现);全部 115 Kind 覆盖,
  E0034 需完全限定。
- TreePath.get_path/get_path_in_path 已按 Java 源码回填(PathFinder 结构 + 短路等价),
  移除 should_panic 标记。

### 14.6 SimpleTreeVisitor 契约

- trait SimpleTreeVisitor<R: Clone, P: Clone>: TreeVisitor<R,P>;R: Clone 用于默认值 clone,
  P: Clone 用于 visit_all 复用参数。
- Java nullable DEFAULT_VALUE -> default_value() -> Option<&R>(None 时 default_action panic,
  对应 Java 返回 null);default_action(&mut self, tree: &dyn Tree, p: P) -> R。
- Java final visit(Tree,P)/visit(Iterable,P) -> visit(Option<&dyn Tree>,P)->Option<R> 与
  visit_all(&[Rc<dyn Tree>],P)->Option<R>;单参便利重载不移植。
- 实现者写空 impl TreeVisitor<R,P>(§14 panic 默认,dispatch 走 SimpleTreeVisitor 方法)。
- dispatch_simple_tree_visitor 为第五份 dispatch(115 Kind,返回 R,OTHER -> visit_other,
  完全限定避免 E0034)。

### 14.7 Trees / DocTrees / DocTreePath 契约

- Trees(19 方法)trait:Java 重载映射为不同 Rust 名(get_tree_for_type/_for_method/_for_annotation/
  _for_annotation_value;get_path_for_element/_for_annotation/_for_annotation_value;
  is_accessible_member),逐方法注明 Java 原名;instance(CompilationTask/ProcessingEnvironment)
  静态工厂 omit,TODO(P6)。
- DocTrees: Trees(协变重写 Java getSourcePositions -> 重声明 get_source_positions_doc);
  重载改名规则同上;java.text.BreakIterator 两个方法 omit(不在移植面);getDocTreeFactory
  待 DocTreeFactory 移植后回填;静态工厂 omit(TODO P6)。
- DocTreePath:具体类 -> struct(tree_path: Rc<TreePath>, doc_comment: Rc<dyn DocCommentTree>,
  leaf: Rc<dyn DocTree>, parent: Option<Rc<DocTreePath>>);new_child 对 DOC_COMMENT 子节点
  panic IllegalArgumentException(照抄 Java);get_path/get_path_in_path 待 DocTreePathScanner
  批次回填(TODO,测试锁定)。
- P6 依赖累积清单(STATUS 跟踪):JavacTask.instance/getElements/getTypes;Trees.instance(×2)/
  getJavacTrees;DocTrees.instance(×2)/getDocTreeFactory/getBreakIterator;DocTreePath 路径搜索;
  javax.lang.model.util.Elements/Types。

### 14.8 DocTreeFactory 契约

- 42 个方法 1:1(40 个 new* + at + getFirstSentence);Java default 方法按 Java 行为 panic
  (newReturnTree(boolean,List) 在 is_inline=true 时 panic,false 时转发;newSummaryTree(List)
  panic,消息照抄)。
- at(int):Java 返回 this 供链式;Rust dyn trait 不能返回 Self -> fn at(&self, pos: i32) -> ()
  且实现用内部可变状态(Cell<i32>);选 &self 使 Rc<dyn DocTreeFactory> 可直接调用(登记偏离)。
- 参数映射:单棵树/Name -> Rc<dyn X>;List<? extends DocTree> -> &[Rc<dyn DocTree>];
  List<DocTree> 返回 -> Vec<Rc<dyn DocTree>>;String -> &JavaString;int -> i32。
- 重载改名:newDocCommentTree(List,...) -> new_doc_comment_tree_with_preamble;
  newReturnTree(boolean,List) -> new_return_tree_with_inline。
- newErroneousTree 的 diag 按 Javadoc 映射 Option<Rc<dyn Diagnostic<JavaFileObjectRef>>>;
  既有 ErroneousTree.get_diagnostic 非 Option(上游接口自身不一致,保持原样)。
- DocTrees.getDocTreeFactory 已回填(返回 Rc<dyn DocTreeFactory>);实际实现(DocTreeMaker/
  JavacTrees)属 P6 API 层。

### 9.1 诊断/消息层适配(ADR 0004)

- api/Messages、api/Formattable、api/DiagnosticFormatter 为 1:1 trait 接口层。
- util/message_format.rs:java.text.MessageFormat 适配(仅上游实际使用的 {n} 与单引号语义;
  choice/number/date/time 上游零使用,出现即 panic 并登记;oracle = 本机 JDK 17 采集数据)。
- util/javac_messages.rs:JavacMessages 1:1;资源接入 resources::compiler_properties
  (COMPILER_MESSAGES);多语言资源由 xtask 生成静态表(zh_CN/ja);bundleCache 用 RefCell
  (诊断层内部可变性白名单,对应 Java SoftReference 缓存);locale 回退:语言-国家-变体 ->
  语言 -> base(英文)。
- java.util.Locale 最小映射(Locale{tag},显示/语言/国家;默认 en;环境 locale 探测 TODO)。
- ResourceBundle -> 静态候选表链(具体 locale -> 语言 -> base);locale 匹配语义:
  language-country-variant -> language-country -> language -> base;翻译缺键回退英文 base。
- 生成的本地化资源文件:crates/javac-core/src/resources/{launcher_properties, javac_properties,
  compiler_zh_CN, compiler_ja, launcher_zh_CN, launcher_ja, javac_zh_CN, javac_ja}.rs
  (xtask gen-resources 生成;8 表 2302 键;确定性生成;resources/mod.rs 手写声明,
  zh_CN 文件用 #[path] 保留大写文件名)。
- Log 输出通道:PrintWriter -> Rc<RefCell<dyn Write>>(ADR 0004 白名单)。
- 诊断参数在 P4 保留原始对象语义与身份,禁止以预格式化文本替代(ADR 0002 修订 C)。
- JavacMessages 的 JCDiagnostic 相关成员(DiagnosticInfo/getDiagFactory/含 Options 的构造器)
  在 D5 用真实 JCDiagnostic 补全(文件 TODO + STATUS 登记),不得写替代实现。

### 9.2 P1 树层表示(ADR 0005)

- JCTree 用 arena+ID + 外层 struct{tag/pos/ty/kind} + enum TreeKind(实测 67 个具体节点类;
  当前已实现 63,JCAnnotation/JCExports/JCOpens/JCRequires 待 code/Attribute+Directive);
  子节点用 TreeId/Option<TreeId>,列表 List<TreeId>,7 个带 Symbol 字段的节点用 Option<SymbolId>。
- 内部保留 JCTree.Tag(实测 105 常量,ADR 锁定)与 com.sun.source Tree.Kind(115)双表示;
  tag_to_kind 穷尽 match + 115 覆盖矩阵测试;内部节点(TYPEBOUNDKIND/LETEXPR 等)无 Kind 时
  行为需用 Java 内部 API 实测确认并登记。
- P1 内部不实现 Rc<dyn Tree> 视图;内部遍历用 TreeKind match + as_xxx 内部访问器;
  P6 由 ApiTree 视图适配实现全部节点 trait(getters 逐方法对照 JCTree 实现,kind 不匹配 panic
  等价 ClassCastException);P1 不写简化 getter。
- Pretty 对照 oracle:开发期 Java 程序(add-exports 调内部 Pretty)生成对照数据并入库;
  运行期零 JVM;$(-Xprint) 不是 Pretty 输出(javac 走 PrintingProcessor),不得用作 oracle。
- TreeMaker 保留 TreeFactory trait;parser 不写 sym/type(Enter/Attr 阶段写,Option<ID>);
  Visitor/遍历方法带 &mut CompilerContext;遍历前 clone List(共享节点,语义不变)。

### 9.3 parser 层表示(ADR 0006)

- parser 13 文件 1:1 映射;TokenKind 实测 114 值(实测 2026-xx;EOF/CUSTOM 不出现在正常 token 流)
  保持 Java 原名(non_camel_case 白名单),
  name/tag 用 kind_name()/kind_tag() 表达;Token struct + enum TokenValue{None,Named(Name),
  String(JavaString),Numeric(JavaString,i32)} 替代 Java 类层次;
- UnicodeReader/JavaTokenizer 输入为 JavaString/UTF-16 code unit(§7);词法 oracle 为开发期
  Java 程序 add-exports 调内部 Scanner 生成数据入库(运行期零 JVM);
- JavacParser S7a/S7b 两步但每步语义 1:1(未覆盖语法属里程碑中间态,最终 S7b 验收);
  错误恢复(RECOVERY_THRESHOLD=50/skip 四条件/去重)照抄;
- EndPosTable 实现类随 parser 文件同文件 1:1(Simple 用 IntHashTable<TreeId,i32>);
- LazyDocCommentTable 的 Entry.tree 延迟缓存用 RefCell(诊断/注释层内部可变性白名单,
  借用窗口短、无重入);Doc 注释解析依赖 tree S8 Doc 树族与 javac-source doctree;
- Source/Preview/Lint 随 P1 前移实现必须 1:1 完整(最小集指范围,不是简化)。

### 9.4 code/Type 与 Symbol/Scope 骨架适配

- Type:Java 子类层次 -> 外层 Type{tag,tsym,kind} + enum TypeKindData 18 变体;子类多态覆盖用
  match kind 表达;Type 构造的 TypeMetadata 参数未移植(metadata 字段/相关方法待 Attribute 批次);
  返回 this 的方法在 arena 模型需 TypeId 参数(暂缺并登记);匿名子类(ArrayType.makeVarargs)
  的特殊覆盖待设计;静态实例(noType 等)待 Symtab;BottomType package-private -> 公开变体(便于 Types);
  UndetVar 随 P4 推理补。
- 待依赖清单(Type):TypeMetadata/Attribute、Symbol 方法、Types/Infer/Printer、递归访问器(需
  TypeId+arena)、Visitor/StructuralTypeMapping;文件内 TODO 逐项跟踪。
- Symbol/Scope:同样 1:1 字段与子类;Scope 的桶遍历顺序必须与 javac 一致(成员顺序可能影响
  诊断/字节码顺序),用 Name 的 index 哈希 + 上游探测算法表达,不得用随机种子 HashMap 直接替代;
  若暂时使用 HashMap,必须证明不改变可观察顺序或在所需处模拟上游桶序。

### 9.5 Symbol/Scope 适配

- Symbol:外层 struct + SymbolKind enum(Delegated/Type/TypeVariable/Package/Class/Var/
  RecordComponent/Param/Binding/Method/MethodHandle/Operator;ModuleSymbol/RootPackage/
  Dynamic* 待 Directive/LoadableConstant 骨架);构造的 completer/metadata 暂缺(待移植)。
- Kinds.Kind 暂放 symbol.rs 的 kinds 子模块,后续迁移 code/kinds.rs(迁移时保持内容 1:1)。
- Scope:完整复刻上游开放寻址双哈希(Name.index 哈希;不得用随机种子 HashMap 表达顺序);
  table 用 Vec<TableSlot>;迭代顺序=elems/shadowed 链与上游一致;NamedImportScope.name2Scopes
  用 HashMap(上游即 HashMap,顺序无行为影响)。
- Scope 过渡适配(待 CompilerContext 落地替换):Entry 不存 name,方法带 SymbolResolver 与
  self_id: ScopeId;get_symbols 返回 Vec 快照(无并发修改时与 Iterable 等价);
  dup/dupUnshared/leave 需 ScopeArena table 共享设计,已登记未实现(不写替代);
  MethodSymbol.code/defaultValue、ClassSymbol.annotationTypeMetadata、VarSymbol.data 待依赖。

### 9.6 Attribute / Directive 与 JCTree 4 变体(决策)

- Attribute 与 Directive 是 Java 不可变但有身份语义的对象(Java 未重写 equals 的类按引用相等):
  采用第 5/6 种 arena + ID —— AttributeArena/AttributeId、DirectiveArena/DirectiveId
  (扩展 ADR 0001 的 arena 清单;identity 由 ID 表达;值构造后不可变)。
- JCTree 的 4 个被阻塞节点映射:JCAnnotation{attribute: Option<AttributeId>}、
  JCExports/JCOpens/JCRequires{directive: Option<DirectiveId>};其它字段 1:1。
- Attribute/Directive 值结构本身保持 1:1;引用点用 ID;访问经 CompilerContext 访问器
  (ctx.attribute(id)/ctx.directive(id))。
- kindName(Symbol)/typeKindName(Type) 待 Symbol.getKind(owner 链 arena 访问)与 CompilerContext
  落地后补(当前 NOT PORTED 登记,不写替代)。
- Directive 的 ModuleFlags/ModuleResolutionFlags 属 Symbol.ModuleSymbol,随 ModuleSymbol 批次补。

### 9.7 CompilerContext 与 arena 访问器

- CompilerContext(适配文件 crates/javac-core/src/compiler_context.rs)独占持有六种 Arena:
  SymbolArena/TypeArena/TreeArena/ScopeArena/AttributeArena/DirectiveArena;提供
  alloc_xxx(value) -> XxxId 与 xxx(id)/xxx_mut(id) 访问器;越界 debug 断言。
- 借用规则:组件方法带 &mut CompilerContext;禁止从 ctx 同时借出两个 arena 的可变引用(需要时
  拆分字段借用或先取快照);禁止为方便引入 Rc<RefCell> 对象图。
- Scope 的 SymbolResolver/self_id 过渡必须替换为基于 CompilerContext 的访问(不留长期过渡)。
- Symbol.getKind(ElementKind 映射,含 owner 链/flags 判定)、Kinds.kindName(Symbol)/
  typeKindName(Type) 在 CompilerContext 落地后补全,移除 NOT PORTED 标记。

### 9.8 树层适配登记(TreeInfo/Pretty)

- Java 显式判空的列表字段(JCNewArray.elems、JCMemberReference.typeargs、JCModuleDecl.directives、
  JCExports/JCOpens.moduleNames)用 Option<List<TreeId>>(null 与空列表必须可区分,否则输出不同)。
- JCCompilationUnit.doc_comments 用 Rc<dyn DocCommentTable>(共享只读表;无 RefCell)。
- Pretty 输出缓冲为 JavaString(无 IOException);lineSep 用 \n,与 oracle 比较前按归一化规则处理;
  printUnit 的 import 过滤依赖未移植 TreeScanner,当前 TODO panic,不伪造输出。
- Pretty 的 dispatch 暂在 Pretty 内按 Tag match;JCTree.Visitor trait 落地后,后续组件
  (TreeScanner/TreeTranslator/Copier/Attr/Check/Flow)一律 1:1 实现 Visitor,不复制 dispatch。
- TreeInfo 静态方法 -> Rust 自由函数;符号/树访问经 CompilerContext;依赖 Types/AttrContext/
  DiagnosticPosition/TreeScanner 的方法暂缺 + TODO,不写替代。

### 9.9 JCDiagnostic 适配(ADR 0004)

- Java Object[] 参数 -> Vec<DiagnosticArg>,参数保留原始对象身份/类型(Rc<dyn Any> 或具体
  ID/Name/JavaString 包装;ADR 0002 修订 C);EnumSet<DiagnosticFlag> -> Vec<DiagnosticFlag>。
- DiagnosticPosition trait:get_tree() -> Option<TreeId>;get_start/preferred/end 位置按 Java。
- DiagnosticInfo.of 返回基类 + Error/Warning/Note/Fragment 4 个键 newtype(上游子类在 Factory
  键边界表达);JCDiagnostic 实现 javac_tools Diagnostic<Rc<dyn JavaFileObject>>。
- Factory::new 的默认 formatter 需要 BasicDiagnosticFormatter(api 批),未移植前未注入则 panic
  + TODO,不伪造输出;静态 fragment helpers 同样待 api 批。
- DiagnosticSource 未移植前:source-less 诊断 getLineNumber/ColumnNumber 返回 -1(上游
  source==null 路径);getEndPosition 用 DiagnosticPosition fallback;Java 构造检查
  (source==null && pos!=null && preferred!=NOPOS -> IAE)在 DiagnosticSource 移植后恢复。
- LintCategory 暂以最小占位定义在 jc_diagnostic,code/Lint 批次迁移并替换真实类型。
- contract::ErrorInfo 等 -> 真实 DiagnosticInfo 的 From 桥接(P4 契约替换后删除占位文本转换)。

### 14.9 javax.lang.model.util.Elements/Types

- 1:1 trait;Java 重载 -> 不同 Rust 名:get_package_element(_in_module)、get_type_element(_in_module)、
  get_origin/_for_construct/_for_directive、get_declared_type(_in);逐方法注明 Java 原名。
- Java 集合/可变参数映射:Set<? extends X> -> Vec<Rc<dyn X>>(保序);Map -> Vec<(K,V)>;
  TypeMirror... -> &[Rc<dyn TypeMirror>];java.io.Writer -> java_io::Writer;
  Object getConstantExpression -> &Value(封闭表示,非 primitive/String -> panic IAE,同 Java)。
- Elements.recordComponentFor 的 Java default 无法在 Rust 默认实现表达(借用元素经 ElementFilter
  返回引用,无法得到 owned Rc)-> 默认 panic("must be overridden"),P6 实现必须覆盖。
- Elements.Origin 为 elements 模块内嵌套 enum。
- javax.lang.model 接口层已完整(119 个 .rs;遗漏项排查为 0;EXTRA 仅 package-info 文档与
  cfg(test) fixture,已登记)。P6 需要 Elements/Types 的 adapter 实现并覆盖 recordComponentFor。

### 9.10 platform 包契约

- PlatformDescription/PlatformProvider/PlatformUtils 1:1;PlatformUtils.lookup_platform_description
  增加显式 providers 参数(&[Rc<dyn PlatformProvider>])替代 ServiceLoader(运行期零 JVM;
  P6 由服务注册表注入并保持选择算法 1:1:首 ':' 拆分、findFirst、options 透传)。
- Java 嵌套接口 PluginInfo -> Rust 顶层 trait;PlatformNotSupported 为 struct + Display/Error;
  PlatformDescription extends Closeable -> trait 方法 close(&self) -> Result<(), Error>。
- java.util.Comparator<String> -> pub fn numerical_comparator(&JavaString,&JavaString)->Ordering;
  Map<String,String> -> Vec<(JavaString,JavaString)>;Iterable<String> -> Vec<JavaString>;
  java.util.stream 用显式迭代(保持行序)。
- JDKPlatformProvider 分批:S1(现有:选择算法/JVM 无关部分);S2 需 jvm.Target + javac-ctsym;
  S3 需 file/JavacFileManager+Log+Option;S4 JDK home 由 launcher 提供;S5 P6 服务注册表。
  未移植部分 unimplemented!+TODO,不写替代(测试 #[should_panic] 锁定)。

### 9.11 javac-ctsym 适配

- crate javac-ctsym(非 1:1 适配层):对应 javac 的 JDKPlatformProvider ct.sym 容器/版本选择
  (+ CreateSymbols 布局说明);基于 javac-zip。仅容器+版本选择,不解析 class(P2 ClassReader);
  不做 jrt/jimage 与 JavacFileManager 路径。
- 基线实测:lib/ct.sym 15710 条目(13652 .sig + 2057 目录 + 1 system-modules);54 个 release
  section,目录名每字符为 base-36 release(7→'7',10→'A',17→'H');多字符目录为共享签名组;
  含 '-' 的目录跳过。版本选择 = section 名含目标 release 的 base-36 字符(非 "<=N 取最新");
  release 17(H)只有 system-modules,签名由 javac-jimage 当前镜像提供(不伪造)。
- API:open/releases/sections/classes(release)(排除 module-info)/sig_entries/modules/
  get_sig/find_class_any_release/has_system_modules/system_modules。
- 上层接入:S2 由 JDKPlatformProvider 消费 releases()/sections()/modules()/system_modules();
  17 转 jimage。get_sig 对解压失败条目返回 None(与 javac-jmod 一致)。

### 9.12 PoolReader / ModuleNameReader

- PoolReader 1:1 可移植部分:readPool(size/offset/长度/两 slot)、readIfNeeded 缓存与 tag 校验、
  peekName/peekClassName/peekPackageName/peekModuleName、resolve;返回原始 PoolValue 枚举
  (Utf8/Class/NameAndType/Integer/Float/Long/Double/String/Package/Module)替代 Object/Object[]
  惰性值;P2 ClassReader 接线时映射到 ClassReader.badClassFile 与 enterClass/enterPackage/
  enterModule/Type 解析(未移植,TODO 不写替代)。
- ModuleNameReader 1:1(361 行):readModuleName 全校验与错误文本照抄;Java 三入口重载 ->
  read_module_name(Path)/read_module_name_from_java_file_object/read_module_name_from_stream
  (输入用 &mut dyn Read,Java InputStream 的 Rust 表达)。
- 测试用开发期 javap 采集期望常量,运行期零 JVM;测试依赖 javac-classpath 仅 [dev-dependencies]
  (不影响生产依赖图)。
- Names/Symtab 构造注入 -> 方法参数(签名级适配);Java BitSet -> 封闭 TagSet 枚举。

### 9.13 javac-jimage /packages 树

- JImage 暴露 packages()(ImageReader children 顺序)/modules_of_package(package)(资源顺序)/
  packages_of_module(module)(/packages 顺序的反向查询);不构建 Node/Directory 树对象。
- 验收偏差(合理):jimage list 工具过滤 tree-info resources,不输出 /packages;改用
  jdk.internal.jimage.ImageReader(同一参考实现)反射读取 + jrt-fs /packages 树全量比对
  (984 包完全一致),期望值入测试常量,运行期零 JVM。
- packages_of_module 暂未缓存(遍历全部包);若性能敏感后续加惰性缓存(需 JImage 增 RefCell 字段,
  登记白名单)。

### 9.14 Source/Target/Profile/Lint/Preview 适配

- Java enum 构造数据(name/major/minor/min/max/optFragment/optKind/value/targets) ->
  match-based 访问器;Java enum name() -> Display(常量名);name 字段(String) -> &'static str
  (动态值如 multiReleaseValue -> JavaString)。
- EnumSet<LintCategory> -> BTreeSet(唯一 + 枚举声明序迭代);Lint(Lint other) protected 构造 ->
  pub(crate) new_from;Set<Target> -> Option<[Target;1]>(上游仅单元素;DEFAULT null->None);
  HashSet<JavaFileObject> identity 语义 -> Vec<JavaFileObjectRef> + Rc::ptr_eq。
- 重载改名:warnPreview -> warn_preview/warn_preview_at/warn_preview_classfile;
  disabledError -> disabled_error/disabled_error_classfile;Profile.lookup -> lookup/lookup_value。
- Preview 构造的 Context 接线(P6)暂以 pub(crate) new(enabled, force_preview) 适配;
  util/jc_diagnostic 的占位 LintCategory 必须在 Log 批整合时替换为 code/lint.rs::LintCategory。
- 数量断言:Source 16、Feature 38(全量清单/顺序)、LintCategory 31、Target 17、Profile 4。
- 待依赖(文件内 TODO):Options/Option 批(instance 方法)、Log/MandatoryWarningHandler 批
  (Preview 诊断入口)、Attribute/Symtab 批(Lint.augment)、P4 诊断对象契约(嵌套 Fragment/
  FileObject 参数占位文本替换)。

### 9.15 javac-fs 适配

- crate javac-fs 为 java.nio.file 的 Rust 等价平台层(默认 FileSystem 子集):Path 解析/规范化/
  relativize/resolve/startsWith/endsWith/fileName/parent/extension/toUri/fromUri/file scheme;
  Files exists/isDirectory/isRegularFile/readAllBytes/listDirectory/size/lastModified;错误映射
  IOException -> Io,InvalidPathException/IllegalArgumentException 保留 Java 角色。
- oracle:开发期基线 JDK 17.0.19 Windows 默认 FileSystem 采集(35 Path + resolve/relativize/
  startsWith/endsWith/uri/equals 全量),数据入测试常量、运行期零 JVM;Linux 行为按 cfg 实现但
  未在本机采样(CI 需另测,登记)。
- extension 为 javac-fs 辅助(Java 无 getExtension);to_absolute_path/to_uri 用进程 cwd 代替
  user.dir;from_uri 仅 file scheme(其他 -> UnsupportedUri);Path::as_path 经 to_java_string
  构造 PathBuf(std 层会规范化混合分隔符/点组件,文件操作语义等价)。
- Ord 用 (root, 元素) 比较键(Windows 大小写不敏感),非 Java 字节序保证。

### 9.16 Options / Option(CLI 选项层)

- Options 的共享可变性(Java 引用语义)用 RefCell:白名单条目;借用窗口=单次表操作、
  不可重入、不构成对象图;get 返回 String 拷贝(RefCell 不能返回借用);Java Map null value
  不可表达(上游未用);listener 在 notify 内修改列表会 panic(上游无此用法)。
- Option 类名遮蔽 std::option::Option:文件内 use std::option::Option as StdOption 别名,
  对外签名用 StdOption(§3 名称保真与 prelude 冲突登记;调用方同别名或全限定)。
- 裁剪:上游 83 常量保留 39(按 SUPPORT_MATRIX:APT/javadoc 废弃/-h/-s/-J*/doclint/其余 -X),
  裁剪 44;测试断言 39+44=83 且 lookup 不识别裁剪项;被裁剪项不造替代行为。
  补充:Option.MULTIRELEASE 因 file 层 BaseFileManager.handleOption 与多版本 jar(P8)需要已恢复
  (裁决记录:task-57 冲突裁决,2026-xx)。
- 未声明方法(process/handleOption/help/helpSynopsis/showHelp/getPattern/Option.PkgInfo/
  HiddenGroup)依赖 OptionHelper/Log/regex/ModuleDescriptor,待 main/OptionHelper+Arguments+Log 批。
- 重载改名:Options.get(Option)->get_option;isSet/isUnset(Option)->is_set_option/_unset_option;
  (Option,String)->is_set_choice/_unset_choice;getBoolean(String,boolean)->get_boolean_or;
  put(Option)->put_option;Option.lookup(String,Set)->lookup_in。
- ArgKind/OptionKind 为 pub(上游 public);OptionGroup/ChoiceKind 为 pub(crate)(上游
  package-private),保留缩写名 + allow(clippy::upper_case_acronyms);InvalidValueException
  仅 String 构造(cause 由项目错误模型承载)。
- Option 值校验(Source/Target/Profile lookup、--release platform、choices/FILEMANAGER 记账、
  @argfile/argv 扫描)与 Options 表填充属 main/OptionHelper+Arguments 批。

### 9.17 差分运行器(javac-tests)

- 差分基础设施(非移植文件,测试工具):Oracle 三态(diagnostics 记录/corpus baselines/实时
  javac 17);比较退出码+规范化 stdout/stderr+class 清单;输出 DiffReport(限量详情);
  退出码 0 通过/1 差分失败/2 环境缺失。
- oracle 环境固定:-encoding UTF-8 + JAVA_TOOL_OPTIONS 注入 -Dfile.encoding=UTF-8
  -Duser.language=en -Duser.country=US(JVM 目标生效,非 JVM 目标忽略,噪音行被归一化剥离)。
- 目标可为任意 javac 兼容命令行;P9 全量时以 rust-javac 为 --target 直接跑;
  corpus 全量 1955 用例耗时较长,建议 filter/limit 分批。
- diagnostics oracle 的 builtin 用例源码当前未入库(加载时跳过),后续完善任务补入。

## 15. 等价性承诺(强制交付契约)

每个移植批次(任务完成汇报/任务板 complete 前)必须包含以下『等价性承诺』段落,缺一不可:

1. 改动清单:本批改动的每个文件/函数。
2. 逐文件等价声明:对本批每个实现文件声明『与 reference/<上游路径>.java 逐方法对照移植;
   没有简化、没有自创算法、没有自行替换数据结构/控制流、没有行为近似;实现可回溯到上游源码行』。
3. 偏差清单:列出本批唯一允许的偏差(表达映射/适配/未移植依赖),逐项给出:上游位置、
   Rust 位置、为何不能 1:1、登记位置(PORTING_RULES §/文件头/TODO 阶段)。没有偏差就写『无』。
4. 未实现清单:unimplemented!/TODO/NOT PORTED 逐项(含依赖未移植原因与阶段),禁止用
   近似实现填补。
5. 承诺句:『我声明本批所有实现均为上游等价移植;没有为通过编译/测试/赶进度而简化或自创
   算法;若审核发现任何未登记偏差,我将返工修正。』

Lead 审核门槛:没有等价性承诺的批次不验收;有未登记偏差(自创算法/简化/近似)的批次退回返工,
并记录到 STATUS。审核时抽样对照 reference 源码与 oracle 数据。

## 16. 红线(强制,高于一切交付压力)

- 任何替代实现、简化实现、自创算法、行为近似都是项目红线,禁止。没有例外。
- 交付规则:要么不写,要么一次性全部写完。
  * 不写 = 以 unimplemented!/TODO 显式暂缺,并在文件头/任务承诺中逐项登记(依赖未移植/阶段未到),
    不得用近似实现顶替;
  * 写 = 该文件/该方法按上游完整 1:1 写完(全部分支、错误路径、边界、注释语义),不留给以后返工;
  * 禁止'先写简化版以后改'、'半成品方法体'、'临时替代再替换'、'为通过编译/测试而简化'。
- 分批只允许在文件级别(一次完整移植若干个文件);文件内部的方法要么完整写完,要么保持 unimplemented
  暂缺登记,不允许写一半的替代逻辑。
- 每批等价性承诺(§15)必须额外声明:『本批写入的每个方法均完整覆盖上游对应方法的全部分支/
  错误路径/边界;未写的方法以 unimplemented!+TODO 登记;本批无任何简化/替代实现。』
- 审核:Lead 逐方法抽查;发现替代/简化/近似 -> 该文件退回重写(不是打补丁),并记录 STATUS。

## 17. 补丁/修复规则(强制)

任何位置需要打补丁或修复时(为通过编译、修测试失败、修行为等),必须先执行以下判定:

1. **先对照官方实现找差异**:查看该位置在 reference 官方源码(对应文件/方法/调用链)中的实现,逐行比对本地代码与官方的差异。
2. **有差异 -> 传染性重构为官方版本**:若本地与官方不一致,说明根因是本地偏离;必须优先完全重构相关代码
   (包括受该处牵连的调用方/被调用方/数据结构,传染性重构——直到该链路与官方版本结构、行为一致),
   禁止在偏离的代码上打补丁、加特判、加容错来掩盖。
3. **无差异 -> 不在该处打补丁,查其它链路**:若本地与官方在该处完全一致,则问题不在该处;不得在此处
   添加补丁/特判/兜底,应沿调用链、数据来源、上游阶段(其它链路)查找真实根因,并同样按官方实现修复根因处。
4. 禁止的修复方式:局部 hack、按用例特判、try/catch 式掩盖、修改测试预期来迁就实现(除非测试本身与官方语义
   不符且经 Lead 审核确认)、绕过根因在上层加兼容。
5. 每批等价性承诺(§15)需增加一段『修复根因分析』:本批每个补丁/修复对应的官方差异结论(一致/不一致)、
   是否做了传染性重构、为何判定根因在该处而非其它链路。

## 18. 汇报真实性规则(强制)

最后汇报/任务 complete 前不得凭记忆、凭印象或自以为是地下结论;必须重新读相关位置代码审视,
确认无误后才能汇报。具体要求:

1. **重新读代码**:汇报前逐一打开本批涉及的每个文件与关键位置(本地实现 + reference 官方对应位置),
   逐行重新审视;不得引用之前阅读的印象。
2. **重新跑验证**:汇报前重新运行 cargo test / cargo clippy 等命令,以本次实际输出为准;不得引用旧结果。
3. **每条结论必须带证据**:汇报中每个断言(方法完整/行为一致/测试通过/无偏差/根因判定)必须附:
   文件路径+行号、命令与输出摘要、oracle 对照数据(如适用);禁止『大概/应该/我记得/之前是』。
4. **逐项自检清单**:汇报需列出『本批汇报前重读/复核清单』(文件+行号范围),证明结论来自重新审视。
5. **发现不符立即更正**:重新审视时发现与之前说法不符,必须主动更正并说明;隐瞒或沿用旧结论视为违规。
6. Lead 审核:抽查汇报证据;发现断言与代码/输出不符,该批退回,并要求按本规则重做后重新汇报;记录 STATUS。

## 19. 可追溯性承诺(强制)

官方(upstream)的每个实现单元都必须能在本项目找到对应位置;不允许任何实现单元无声消失。
实现单元指:Java 文件/类/内部类/方法/构造器/字段/常量/枚举值/资源键。对应位置分三种:

1. 已移植:对应 Rust 文件 + 函数/常量/变量(汇报时给出文件路径与行号锚点)。
2. 未移植:必须在项目中存在可搜索的登记位置(文件内的 unimplemented!+TODO 行,或
   STATUS/PLAN/文件头的阶段登记清单),并注明依赖原因与阶段;不允许既未移植又无登记。
3. 适配层(标准库/平台类,非 javac 源码):对应适配 crate/文件 + 来源锚点 + PORTING_RULES §9 登记。

承诺要求:
- 每批汇报必须附『官方单元映射覆盖表』:本批涉及的上游文件/方法清单 -> 本地位置(文件:行)或未移植登记位置。
- 对已移植文件,必须能回答:上游该文件的每个方法/字段在本地哪里(或登记在哪);抽查不通过即退回。
- 工具审计:PORTING_MANIFEST 必须能反映所有上游 Java 文件的映射状态;后续将扩展为方法级覆盖审计
  (xtask audit),对已移植文件抽查上游方法名是否在本地出现或有 unimplemented/TODO 登记。
- 违规:未登记地跳过/删除上游实现单元,或声称移植但本地找不到对应物,按 §16 红线处理(退回重写)。

## 20. Lead 审核零信任原则(怀疑一切)

Lead 对执行者的任何汇报持零信任态度:汇报中的断言、数字、映射表、承诺一律不直接采信,必须独立验证。
每批验收必须执行以下检查,缺一不可:

1. **独立读代码对照**:直接打开 reference 官方源码与本地实现,逐方法/逐段对照(覆盖本批关键路径;
   抽样必须包含:核心方法、错误路径、边界、被声称的适配点);发现语义差异、未登记近似或简化即退回。
2. **独立运行验证**:自己运行 cargo test / clippy / audit 等命令,不引用执行者提供的输出;
   注意测试是否真的覆盖新代码(断言是否有效、是否被跳过/忽略)。
3. **独立核对 oracle 与数据**:检查 oracle 数据来源、生成命令、断言值是否真的对应官方行为;
   不接受'测试通过'作为等价性证据,只接受'与官方行为逐项一致'的证据。
4. **检查 git diff 范围**:核对改动是否越界、是否含未登记修改、是否有被回退/覆盖的痕迹(如 clippy --fix 误改)。
5. **抽验映射覆盖表与行号**:随机+重点抽验执行者提供的上游位置/本地位置,发现错行/错映射即要求更正。
6. **记录审核证据**:在 STATUS/审核记录中写入本次核对的 reference 行号、本地行号、命令输出摘要与结论;
   不可验证的汇报结论视为未验证,退回重做。
7. 执行者的汇报必须可复现、可核对;无法提供位置/证据的断言一律不采信。

执行者须知:审核将独立进行,任何'已实现/已对齐/已登记'的说法都需要能被独立验证;
发现未登记偏差时,该文件退回重写(不是打补丁)。

## 21. 质量优先,禁止以预算/上下文/时间为由降低质量(强制)

- 禁止以『上下文快满了』『本轮运行时间太长』『要快点收尾』『省预算/省 token/省时间』等任何理由
  简化实现、跳过验证、省略对照、减少测试、提前收尾或交付半成品。
- 预算、时间、上下文长度都不构成降低质量或省略步骤的依据;必须按上游完整 1:1 做完、按规则完整验证、
  按 §20 完成独立审核并留证据。
- 若任务确实大到一轮做不完:按文件级/阶段级继续分批,每批保持完整与可编译;分批原因是依赖与结构,
  不是预算;不得用『分批』掩盖半成品。
- 交付目标是可用的最完整实现(最终能编译 Java 17、行为与 javac 对齐),不是『差不多能跑』。
