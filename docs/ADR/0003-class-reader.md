# ADR 0003:ClassReader / class 文件读取层设计

- 状态:已接受(Lead 审核通过;裁决与修订见文末"审核裁决记录")
- 基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)
- 相关:ADR 0001(arena + ID)、ADR 0002(诊断资源类型化访问器)、docs/PLAN.md P2/P3

## 背景

javac 通过 ClassFinder 驱动 ClassReader 把 class 文件读成内部表示(ClassSymbol 及其
scope 中的成员/类型)。本 ADR 只覆盖读取层(ClassReader/PoolReader/ClassFile/
ModuleNameReader/Profile 与数据源接入),不涵盖查找策略的完整实现(ClassFinder 作为
边界说明)。

上游文件与规模(以 reference/jdk17u sparse 检出的 982d53c 为准):

| Java 文件 | 行数 | 角色 |
|---|---:|---|
| src/jdk.compiler/.../jvm/ClassReader.java | 3286 | 读取 class 文件 |
| src/jdk.compiler/.../jvm/PoolReader.java | 356 | 常量池解码 |
| src/jdk.compiler/.../jvm/ClassFile.java | 193 | 魔数/常量 tag/Version/名称转换 |
| src/jdk.compiler/.../jvm/PoolConstant.java | 220 | 池常量类型(NameAndType 等) |
| src/jdk.compiler/.../jvm/ModuleNameReader.java | 156 | 从 module-info.class 快速读模块名 |
| src/jdk.compiler/.../jvm/Profile.java | 105 | compact profile(ClassReader 构造依赖) |
| src/jdk.compiler/.../code/ClassFinder.java | 约 900 | 查找/补全驱动、BadClassFile、ct.sym 补充 flags |
| src/jdk.compiler/.../platform/JDKPlatformProvider.java | 约 370 | ct.sym 平台文件管理器 |

注:Lead 任务描述中的 ClassReader.java 2954 行与检出的基线不一致,本设计以实际检出为准。

读取入口链路(全部在 code/ClassFinder 与 jvm/ClassReader 之间):

1. ClassFinder.complete(sym):annotate.blockAnnotations(); completeOwners/completeEnclosing;
   fillIn(c)。
2. fillIn:若 c.classfile 是 Kind.CLASS,调用 reader.readClassFile(c),再
   c.flags_field |= getSupplementaryFlags(c)(ct.sym 补充 flags,见数据源一节);
   否则转 sourceCompleter(源码读取,不在本 ADR 范围)。
3. ClassReader.readClassFile(2955):
   - 设置 currentOwner/currentClassFile,清空 warnedAttrs/target/repeatable;
   - buf.reset(); buf.appendStream(c.classfile.openInputStream());
   - readClassBuffer(2912):读魔数(0xCAFEBABE)、minor/major 版本、版本上下界检查、
     preview 检查;PoolReader.readPool;初始化 signatureBuffer;readClass(c);
   - 读完后处理 missingTypeVariables/foundTypeVariables 的 types.subst/substBounds;
   - 设置 AnnotationTypeMetadata(注解类型时用 CompleterDeproxy,否则 notAnAnnotationType);
   - module-info 时把 interimUses/interimProvides 交给 UsesProvidesCompleter;
   - 异常映射:IOException/ClosedFileSystemException -> unable.to.access.file;
     ArrayIndexOutOfBoundsException -> bad.class.file。
4. readClass(2787):
   - 分配 members scope;typevars = typevars.dup(currentOwner);entertypevars(enclosing);
   - 读 flags/this_class(校验 c == self;module-info 校验 owner 是 MDL 且版本 >= 53);
   - 先跳过 fields/methods 到 class attrs 读取(readClassAttrs):Signature/InnerClasses/
     EnclosingMethod/SourceFile/Module/ModuleResolution/Record/PermittedSubclasses;
   - 回到 class 头读 super/interfaces(supertype_field/interfaces_field 为空时用描述符,
     Signature 属性已填时优先);再读 fields(readField 2509)/methods(readMethod 2520);
     record 时补 accessor lookup;typevars = typevars.leave()。

ClassReader 依赖的外部组件(全部为 P2/P3 交付物):
Annotate(comp)、Attribute/TypeAnnotationPosition/TargetType、ClassFinder.BadClassFile、
Source.Feature、Preview、Profile、Symtab/Types/Flags/Kinds/Scope/Directive/Symbol/Type、
JCDiagnostic/Log/DeferredCompletionFailureHandler、Names/Name/List/ListBuffer/ByteBuffer/
Convert/Assert/Options/Context、JavaFileObject/JavaFileManager/BaseFileManager/
PathFileObject、ClassFile.Version/PoolConstant、main.Option、以及我们 task-19 生成的
resources::compiler_properties(Errors/Warnings/Fragments 访问器)。

当前 rust-javac 现状(截至本 ADR):
- 已完成:P0 基础设施(javac-jimage/javac-jmod/javac-zip 读取器、javac-tests 差分基座、
  诊断访问器生成器)、P1 的 util/parser/tree 一部分、javac-tools 的 javax.tools 接口层
  (含 FileObject/JavaFileObject/JavaFileManager trait);
- code/ 只有 bound_kind/flags/target_type/type_tag;file/ 与 comp/ 尚无实现;
  util 尚无 Context/Log/Options/JCDiagnostic;P2 未开始。

## 决策

### 1. Rust 模块映射

| Java | Rust |
|---|---|
| jvm/ClassReader.java | crates/javac-core/src/jvm/class_reader.rs |
| jvm/PoolReader.java | crates/javac-core/src/jvm/pool_reader.rs |
| jvm/ClassFile.java | crates/javac-core/src/jvm/class_file.rs |
| jvm/PoolConstant.java | crates/javac-core/src/jvm/pool_constant.rs |
| jvm/ModuleNameReader.java | crates/javac-core/src/jvm/module_name_reader.rs |
| jvm/Profile.java | crates/javac-core/src/jvm/profile.rs |
| jvm/Target.java | crates/javac-core/src/jvm/target.rs(Profile 依赖) |
| code/ClassFinder.java | crates/javac-core/src/code/class_finder.rs(边界说明) |
| code/BadClassFile(ClassFinder 内部类) | code/class_finder.rs 的 BadClassFile |
| util/JCDiagnostic(最小) | util/jc_diagnostic.rs(P2 最小,P4 完整) |
| util/Log(最小) | util/log.rs(P2 最小) |

1:1 规则:文件头来源锚点、保留英文 Javadoc、方法名 snake_case、控制流/顺序照抄;
ClassReader 的私有静态嵌套类(SourceFileObject/ProxyType/InterimUsesDirective/
InterimProvidesDirective/UsesProvidesCompleter)按同样顺序放在同一文件或紧邻支持模块;
Proxy 类(Enum/Class/Array/CompoundAnnotationProxy/TypeAnnotationProxy)放在
class_reader.rs 内的支持结构(与上游同一文件)。

### 2. 数据源(jimage / jmod / zip / ct.sym)

ClassReader 不感知任何容器格式:readClassFile 只做 JavaFileObject.openInputStream() ->
buf.appendStream(...) 的字节读取;容器解析全部在 file/数据源层。Rust 侧接口沿用
javac-tools 已移植的 javax.tools 抽象:FileObject/JavaFileObject/JavaFileManager:

- JavaFileObject 提供 open_input_stream() -> Box<dyn Read>(已存在契约)与
  get_name()/to_uri()/get_kind()/get_last_modified();ClassReader 只依赖
  open_input_stream 与 get_kind;
- binary name 推断由 JavaFileManager.infer_binary_name 提供(ClassFinder 使用);
- ClassSymbol.classfile 在 Rust 侧为 Option<Rc<dyn JavaFileObject>>(javac-tools
  的引用约定)。

数据源与既有 crate 的衔接:

| 数据源 | 容器 | Rust 读取器 | 说明 |
|---|---|---|---|
| JDK 17 平台类(默认) | lib/modules(jimage) | javac-jimage(已交付,P0) | JImage::get_class_bytes(module, binary_name) -> Vec<u8>;jimage 的 module/resource 表已对照 jimage list 验证 |
| jmods 目录(--system) | .jmod(4 字节头 + zip) | javac-jmod(已交付,P0) | JMod::get_class_bytes / module_info_bytes;Entry 清单已对照 jmod list 验证 |
| jar / 目录(class path / module path) | zip / 文件系统 | javac-zip(已交付,P0)+ std::fs | ZipArchive::read_entry;目录遍历生成 JavaFileObject |
| ct.sym(--release / 指定 system) | 多版本 zip(JDK zipfs) | javac-zip | 见下 |

ct.sym 细节(platform/JDKPlatformProvider.java + code/ClassFinder 的临时替代):
- ct.sym 是 zip 文件,包含多个 release 目录(如 7、8、...、17),每个目录内是
  module/package/Class.class 布局;
- 平台 provider 打开 ct.sym(FileSystems.newFileSystem)后:
  * 无模块版本(hasModules=false):把含 ctSymVersion 且不含 "-" 的 section 下的
    module 目录加入 PLATFORM_CLASS_PATH;
  * 有模块版本:优先读 section/system-modules(每行一个模块名)并映射到 jrt:/modules;
    否则把 module 目录按模块名设置 SYSTEM_MODULES location;
  * 用 SigJavaFileObject 包装(强制 Kind.CLASS,inferBinaryName 转委托);
- code/ClassFinder 的"临时 ct.sym 替代":当默认 bootclasspath 且
  JavacFileManager.isSymbolFileEnabled() 时,用 JRTIndex.getCtSym(package) 得到
  proprietary/minProfile,并映射为 ClassSymbol 的补充 flags(getSupplementaryFlags),
  不再从 ct.sym 类文件读取私有注解;
- Rust 侧:P2 内先定义 CtSym JavaFileManager(多版本目录选择 + system-modules +
  SigJavaFileObject 等价),版本目录选择逻辑 1:1 复刻;JRTIndex 的补充 flags 在 P8
  (多 release)/P9 阶段接入;需要 JavaFileObject 的 getLastModified 用于 ClassFinder 的
  class/source 优先级(preferredFileObject),ct.sym 条目恒为 0 的语义按上游保留。

结论:ClassReader 只需一个 JavaFileObject 字节流契约;P0 的三个读取器在 file 层
包装为 JavaFileObject/JavaFileManager 实现,不在 ClassReader 中做容器分支。

### 3. 常量池模型(PoolReader)

上游机制(PoolReader.java 356 行):
- readPool(poolbuf, offset):第一遍只扫描 pool 条目,记录每个 index 的 offset
  (int[] offsets)并跳过条目(按 tag 的固定宽度;Utf8 用长度;Long/Double 占 2 个索引
  sizeof(tag));遇到未知 tag 抛 bad.const.pool.tag.at;返回 pool 结束 offset;
- 解析是延迟的:readIfNeeded(index, expectedTags) 先查 values[index] 缓存,命中直接返回;
  未命中校验 tag 是否在 expectedTags(BitSet)内,否则 bad.const.pool.tag.at;
  resolve(poolbuf, tag, offset) 按 tag 创建 javac 实体并写回缓存;
- peek 系列(peekClassName/peekPackageName/peekModuleName/peekName)不创建实体,只返回
  NameMapper 结果(用于 ClassFinder/ModuleNameReader 避免过早 enterClass);
- checkIndex:index <= 0 或 >= offsets.length -> bad.const.pool.index;
- resolve 支持的 tag:Utf8/Class/NameandType/Integer/Float/Long/Double/String/Package/
  Module;其它(MethodHandle/MethodType/Dynamic/InvokeDynamic/Fieldref/Methodref/
  InterfaceMethodref)不参与读取(default -> unexpected.const.pool.tag.at)。

Rust 设计:
- PoolReader::read_pool(buf: &ByteBuffer, offset: usize) -> Result<usize, BadClassFile>:
  只构建 offsets: Vec<u32> 与 tag 校验;不解析值;
- 缓存 values: Vec<Option<PoolValue>>;PoolValue 枚举替代 Java Object[]:
  Utf8(Name)、Class(ClassSymbolId)、NameAndType(NameAndType)、Integer(i32)、
  Float(f32)、Long(i64)、Double(f64)、String(JavaString)、Package(PackageSymbolId)、
  Module(ModuleSymbolId);
- NameMapper<Z>:Java 泛型接口 -> Rust 泛型闭包 F: Fn(&[u8], usize, usize) -> Z 或 trait
  NameMapper<Z>(建议 trait,保持上下游命名与调用形态);
- 延迟语义:resolve 才通过 Symtab.enter_class/enter_package/enter_module 在 arena 中创建
  SymbolId(与上游"避免过早创建符号"的注释一致);pool 索引错误、tag 错误、期望 tag 不符
  都转为 BadClassFile(诊断 key 见错误诊断一节);
- 常量如 Integer/Float/Long/Double/String 不创建 Symbol,直接值;String 上游用
  Name.toString()(JavaString),Rust 用 JavaString;
- 性能:offsets 与 values 为 Vec<Option<...>>,延迟解析保证只解析实际使用的条目;
- 迁移差异登记(PORTING_RULES §9):Java Object[] 缓存 -> Rust 枚举;Java BitSet
  expectedTags -> Rust 常量表(如 &[u8] tag 集合或位集);行为一致。

### 4. Signature 解析(ClassReader 的 sigToType 家族)

上游状态机(signature: byte[], sigp/siglimit, sigEnterPhase;signatureBuffer/sbp 用于
classSigToType 拼名字):
- sigToType() 按首个字符分派:
  T(类型变量,查找 typevars,enter phase 时返回 noType)、+ (- 通配符 SUPER/BoundKind)、
  *(UNBOUND)、B/C/D/F/I/J/S/V/Z(基本类型从 Symtab 取预定义 Type)、
  L(classSigToType;若后面跟 '.' 则报 deprecated inner class signature syntax)、
  [ (ArrayType(sigToType(), syms.arrayClass))、((方法签名:参数列表 + 返回类型 +
  ^ 抛出类型,throws 里的 TypeVar 打 THROWS 标志,new MethodType(...))、
  < (类型参数:sigToTypeParams + 方法类型 -> ForAll);其它 -> bad.signature;
- classSigToType():解析 L...; 与泛型嵌套类(outer ClassType 与 '.'->'$'),
  旧式 GJC 签名兼容(Lfoo/Outer<...>;.Lfoo/Outer$Inner<...>;);
  结尾用 enterClass 取 ClassSymbol,返回 t.erasure(types)(无 outer)或
  new ClassType(outer, args, t);内联匿名 ClassType 覆盖 getEnclosingType(惰性
  tsym.apiComplete 后按 typeParams 数量决定 subst/erasure);
- sigToTypes(terminator):读到终止符的 Type 列表(上游用 List.of(null) 头节点技巧);
- sigToTypeParams():两遍:先 sigEnterPhase=true 建 TypeVar 并 enter 到 typevars,
  再 sigEnterPhase=false 找同一 TypeVar 并 setBounds(支持 bounds 引用其它 typevar);
- sigToTypeParam():name: + bounds(允许 "::" 起始的 allInterfaces 标记);
- findTypeVar():在 typevars scope 中找;readingClassAttr 期间找不到时创建"缺失
  typevar"并记入 missingTypeVariables(读完后由 readClassFile 用 types.subst/
  substBounds 替换),否则 bad.class.signature/undecl.type.var;
- 错误 key:bad.signature、bad.class.signature、deprecated inner class signature
  syntax、undecl.type.var。

Rust 设计:
- 保持字段式状态机:ClassReader 上的 signature: Vec<u8>/sigp/siglimit/sig_enter_phase/
  signature_buffer/sbp;方法签名 fn sig_to_type(&mut self, ctx: &mut CompilerContext) ->
  Result<TypeId, BadClassFile>;
- Type 构造通过 arena:ClassType/ArrayType/MethodType/TypeVar/ForAll/WildcardType 在
  TypeArena 分配并返回 TypeId;Symtab 提供预定义类型 id(byteType...arrayClass/
  methodClass/boundClass/botType/objectType/methodClass);
- enterClass -> Symtab.enter_class(ctx, current_module, name)(返回 ClassSymbolId);
- ClassType 的惰性 enclosing 计算(Rust 无匿名子类):在 ClassType 结构内放
  enclosing: Option<TypeId>、completed: bool 字段,提供
  ClassType::enclosing_type(ctx) 方法复刻 getEnclosingType 的分支
  (tsym.apiComplete 后按 allparams 长度 erasure/subst/setEnclosingType);
  ADR 0001 规则 5 允许惰性字段按上游时机填充;
- typevars 为 ScopeId(WriteableScope 等价),sigToTypeParams 两遍处理用
  sig_enter_phase 字段;settle bounds 用 Types::set_bounds;
- missingTypeVariables/foundTypeVariables 保存 TypeId 列表,readClassFile 收尾时
  types.subst/subst_bounds(与上游一致);
- 差异登记:Java 内联匿名类重载 getEnclosingType -> Rust 方法 + 字段;其余 1:1。

### 5. 与 arena + ID 的交互契约

ClassReader 是"写入器":读 class 文件把数据写进 CompilerContext 的 arena。契约:

1. 输出对象:
   - ClassSymbol -> SymbolArena(SymbolId);字段:name/fullname/flags_field/type(TypeId)/
     members(ScopeId)/classfile(Option<Rc<dyn JavaFileObject>>)/sourcefile(Option<
     Rc<dyn JavaFileObject>>)/completer/record_components/permitted 等;
   - Integer/Long/Float/Double/String 常量值(Attribute.Constant 的数据)直接存值;
   - Scope -> ScopeArena(ScopeId):members_field/typevars;WriteableScope::enter/
     dup/leave/remove/find_first 由 Scope 模块提供;
   - Type -> TypeArena(TypeId):ClassType/ArrayType/MethodType/TypeVar/ForAll/
     WildcardType 等结构;
   - Attribute/TypeCompound/Enum/Class/Array -> code/Attribute(值类型,化合物内含
     TypeId/JavaString/Name);
   - Directive 家族(Requires/Exports/Opens/Uses/Provides)-> code/Directive 值类型;
   - Annotate 任务:注解代理先存值(Proxy 枚举),完成后写回 Symbol 的 annotations。
2. 访问方式:ClassReader 方法接 &mut CompilerContext(或按 ADR 0001 规则 3 的
   ctx 参数形式),通过 ctx.sym(id)/ctx.sym_mut(id)/ctx.typ(id)/ctx.typ_mut(id)/
   ctx.scope(id) 读写;禁止 RefCell 对象图(ADR 0001 规则 4);
   需要同时写多个 arena 时用字段拆分借用(ctx.sym 与 ctx.typ 是不同字段)。
3. 状态字段:currentOwner: Option<SymbolId>、currentModule: Option<ModuleSymbolId>、
   currentClassFile: Option<Rc<dyn JavaFileObject>>、typevars: Option<ScopeId>、
   signature 状态、参数名/注解数组、filling、warnedAttrs 等放在 ClassReader struct
   (由 CompilerContext 持有,Context 单例语义),方法 &mut self 与 &mut CompilerContext。
4. 惰性/可变字段:与 ADR 0001 一致——Type 构造后按 javac 语义不修改(惰性字段除外:
   ClassType.typarams_field/supertype_field/interfaces_field 读取时写入,先设置后不再
   修改;enclosing 惰性计算按上游时机)。Symbol.flags_field/members/classfile 在读取阶段
   写入(与上游相同的写入点)。
5. completer 链:ClassReader 不负责补全生命周期;ClassFinder.complete 驱动
   c.complete()(Symbol completer 回调),BadClassFile 作为 CompletionFailure 抛出,
   由 DeferredCompletionFailureHandler(dcfh)分流;Rust 侧 completer 用
   Box<dyn FnMut(&mut CompilerContext, SymbolId) -> Result<(), CompletionFailure>>
   或任务枚举(ADR 0001 平台层例外之外不引入 Rc<RefCell>)。
6. 重入保护:上游 filling 标志禁止读 class 期间再次 fill;Rust 保持同一标志与
   断言(Assert.error 等价)。

### 6. 属性读取(AttributeReader 表)

上游 initAttributeReaders 注册表(按版本门槛与 CLASS/MEMBER 适用范围):
Code(仅 saveParameterNames 时读,否则跳过)、ConstantValue、Deprecated、Exceptions、
InnerClasses(module_info 时 skipInnerClasses)、LocalVariableTable(参数名)、SourceFile
(AUXILIARY 判定)、Synthetic、EnclosingMethod、Signature(类/成员,类时先读 class attr
再回填)、AnnotationDefault、RuntimeInvisible/RuntimeVisibleAnnotations、
RuntimeInvisible/RuntimeVisibleParameterAnnotations、Annotation、Bridge、Enum、Varargs、
RuntimeInvisible/RuntimeVisibleTypeAnnotations、MethodParameters(参数名/访问标志)、
Module(仅 allowModules;requires/exports/opens/uses/provides + 校验)、ModuleResolution、
Record(仅 allowRecords)、PermittedSubclasses(仅 allowSealedTypes)。

版本门槛:低于属性引入版本时不读;若 lintClassfile 开启则打印 Warnings.FutureAttr
(每个属性名只警告一次,记在 warnedAttrs)。

Rust 设计:readAttrs(sym, kind) 1:1;attributeReaders 用 HashMap<Name, AttributeReader>。
Java 的 AttributeReader 抽象类/匿名子类 -> Rust trait 或闭包表(建议 trait
AttributeReader { fn accepts(&self, kind) -> bool; fn read(&mut self, ctx, sym, len) });
版本来判断用 ClassFile.Version 枚举常量;错误/警告用 P2 最小 Log。

readCode 上游当前实现只跳过(读 max_stack/max_locals/code_length/exception_table 后
readMemberAttrs 并返回 null);参数名来自 MethodParameters/LocalVariableTable
(Code.width 计算槽位宽度,long/double 占 2 槽)。

### 7. 注解与类型注解

上游结构(ClassReader 内约 1200 行):
- 读取:readAnnotations/readCompoundAnnotation/readAttributeValue/readTypeAnnotation/
  readPosition/readTypePath;代理类型:EnumAttributeProxy/ClassAttributeProxy/
  ArrayAttributeProxy/CompoundAnnotationProxy/TypeAnnotationProxy(均继承 Attribute/
  值类型);
- 延迟完成:attachAnnotations -> annotate.normal(new AnnotationCompleter(sym, proxies));
  attachAnnotationDefault -> AnnotationDefaultCompleter;attachTypeAnnotations ->
  TypeAnnotationCompleter;注解类型(@Target/@Repeatable)元数据读取后由
  CompleterDeproxy(AnnotationTypeCompleter)完成;module-info 的注解用 ProxyType
  延迟 resolve;
- 特殊注解处理:@Target/@Repeatable 原型保存;Deprecated(@Deprecated|@DeprecatedAnnotation|
  forRemoval)、PreviewFeature(PREVIEW_API/PREVIEW_REFLECTIVE)、ValueBased、
  @Profile(NOT_IN_PROFILE)、@Proprietary(PROPRIETARY)等标志写入;
- type annotations:addTypeAnnotationsToSymbol/TypeAnnotationSymbolVisitor/
  TypeAnnotationStructuralTypeMapping 把 TypeAnnotationPosition(TargetType/
  TypePathEntry)应用到对应类型/符号;参数注解存 parameterAnnotations 数组,
  setParameters 时挂到 ParamSymbol;
- 注解值读取:readAttributeValue 的 tag 集合(B/C/D/F/I/J/S/Z/s/e/c/[/@)与
  Attribute.Constant/Enum/Class/Array/Compound 对应。

Rust 设计:
- Proxy 用值类型 enum(Rust 无继承):AnnotationProxy(Compound/Enum/Class/Array)、
  TypeAnnotationProxy { position: TypeAnnotationPosition, annotation: AnnotationProxy };
- Annotate 队列:comp/annotate.rs 提供 Vec<AnnotateTask>(任务枚举),ClassFinder.complete
  在读取完成后 flush(与上游 blockAnnotations/unblockAnnotationsNoFlush/flush 时序一致);
  任务闭包/枚举持有 SymbolId + Proxy 值,不持有跨 arena 可变引用;
- TypeAnnotationPosition/TargetType 属 code 包(P3 计划),需先移植;
- 注解 Attribute 类型(code/Attribute 家族)在 P3;ClassReader 的 Proxy ->
  Attribute 的 deproxy 在 Annotate 完成后进行;
- 建议分期:P2a 基础读取(无注解,未知属性跳过);P2b 注解/type annotations/参数名/
  module 属性/record/sealed。理由:注解是 ClassReader 最大复杂区(约 1200 行),
  且依赖 code/Attribute + Annotate;基础读取已能支撑 javap 对照验证。

### 8. 错误诊断

上游:badClassFile(key, args) 返回 ClassFinder.BadClassFile(CompletionFailure 子类,
带 currentOwner.enclClass/currentClassFile/diagFactory.fragment(key,args)/dcfh);
ClassFinder.fillIn 捕获 BadClassFile 时 purge 部分初始化的符号后重抛。

ClassReader 的 badClassFile key(源码实测 29 处调用,唯一 key):
deprecated inner class signature syntax、bad.signature、bad.class.signature、
undecl.type.var、bad.constant.value.type、bad.constant.range、bad.constant.value、
module.name.mismatch、bad.requires.flag、module.non.zero.opens、bad.enclosing.class、
bad.enclosing.method、bad.runtime.invisible.param.annotations、bad.type.annotation.value、
invalid.default.interface、invalid.static.interface、malformed.vararg.method、
method.descriptor.invalid、class.file.wrong.class、anachronistic.module.info、
module.info.definition.expected、module.info.invalid.super.class、
illegal.start.of.class.file、wrong.version、unable.to.access.file、bad.class.file。

PoolReader 的 key:unexpected.const.pool.tag.at、bad.const.pool.tag.at、
bad.const.pool.index。

其它诊断(Log):
- log.warning(Warnings.BigMajorVersion / Warnings.FutureAttr / Warnings.AnnotationMethodNotFound /
  Warnings.AnnotationMethodNotFoundReason / Warnings.UnknownEnumConstant /
  Warnings.UnknownEnumConstantReason);
- log.error(Errors.CantAttachTypeAnnotations);
- preview.disabledError(currentClassFile, majorVersion)/preview.warnPreview(c.classfile, majorVersion)。

Rust 设计:
- key 都是 Fragments 前缀(compiler.misc.*):task-19 生成的 resources::compiler_properties
  的 fragments 访问器(如 fragments::bad_signature(...))直接构造 DiagnosticInfo;
- ClassReaderError/BadClassFile 保存 DiagnosticInfo(prefix/kind/code/args)与
  currentClassFile;ClassFinder.BadClassFile 等价类型实现 CompletionFailure 协议
  (dcfh 分流),补全失败诊断由 P4 的 JCDiagnostic/Log 输出;
- P2 需要最小 Log:log.error/warning(DiagnosticInfo)/log.useSource/printVerbose;
  最小版可先只构造诊断并记录(输出格式化 P4 完整);
- 校验:测试用错误 key 断言(见测试策略),并对照 javac 的诊断文本(task-19 的
  oracle harness 可编译触发)。

### 9. 测试策略

分层(全部复用已有基础设施:javac-jimage/javac-jmod/javac-zip 的 oracle 数据、
javac-tests 的差分基座与 normalize/compare、task-19 资源访问器):

1. 单元测试(不依赖真实 JDK):
   - PoolReader:构造最小常量池字节(各 tag/索引/long-double 双槽/未知 tag/tag 不匹配/
     index 越界),断言解析结果与错误 key;
   - Signature:内联签名覆盖全部 token(T/+/&/*/-/基本类型/L/[/(/<;)、泛型嵌套类、
     旧式 GJC 兼容签名、deprecated inner class 报错、两遍 typevar bounds、
     missing typevar 延迟替换、bad.signature/bad.class.signature 错误;
   - 版本/魔数:非法魔数、版本过旧/过新(含 BigMajorVersion warning)、preview minor
     版本(0xFFFF)分支;
   - 属性分发:构造最小 class 字节,验证 CLASS/MEMBER 属性接受、版本门槛、未知属性
     跳过、FutureAttr 警告(每属性一次);
   - 参数名:MethodParameters 优先、LocalVariableTable 回退、合成 argN 命名、long/double
     双槽。
2. 集成测试(真实数据源,已有 oracle):
   - jmod:java.base.jmod 6508 类全部读取(类数/顶层类名/flags/字段数/方法数/方法签名
     摘要 FNV),与 task-5 的 jmod oracle 数据(jmod list)对照;抽样类断言
     Object.class 的方法/字段/标志;
   - jimage:lib/modules 的 modules/resources(任务-4 oracle)抽样,验证
     ClassReader 输出与 jimage 条目一致(类名/长度);
   - zip:jrt-fs.jar 61 条目中 class 条目解析抽查;
   - 坏 class:截断/错 magic/错版本/错常量池/错签名 -> 断言错误 key(与 javac 对照,
     见 3)。
3. javap 对照(PLAN P2 验证要求):
   - 用 javac-tests 的 Javac oracle harness 调用本机 JDK 17 的 javap:
     javap -p -s -v(抽样)对 java.base 的类输出解析(类修饰/签名/方法描述符/字段/
     注解计数/InnerClasses/EnclosingMethod),与本实现读取结果逐项比较;
   - 全量类签名摘要:对 java.base 全部类生成 (类名, flags, super, interfaces,
     字段/方法描述符/签名) 摘要 hash,与 javap 派生 hash 对照(抽样类逐项,全量 hash);
   - ASCII 输出归一化:复用 task-8 的 normalize/compare(CRLF/路径/编码)。
4. 差分基座(javac-tests):构造使用平台类/classpath/多 release 的测试源,用 oracle
   harness 编译并比较 stdout/stderr/class 清单;P2 内先保证 ClassReader 使能的编译
   输出与 javac 一致(P9 全量差分绿)。
5. ct.sym/多 release(P8 起):--release 8/11/17 编译同一 source,对照 javac 的
   ct.sym 版本目录选择/jrt-fs 行为;ct.sym 读取逻辑单测(版本目录选择/system-modules/
   SigJavaFileObject 语义)。

### 10. 依赖顺序建议(P2 实施顺序)

P2 的关键路径是 ClassReader 依赖的 code 包基础结构,建议按以下顺序推进,每个阶段
可独立验证:

| 阶段 | 内容 | 依赖/验证 |
|---|---|---|
| S0 | jvm/ClassFile(常量/Version/名称转换)、jvm/PoolConstant(池常量类型/tag)、jvm/Profile(需 Target)、jvm/Target | 纯值/常量,单元测试;Profile/Target 被 ClassReader/ClassWriter 共用 |
| S1 | util 最小前置:JCDiagnostic(诊断对象/Factory 最小)、Log(最小 error/warning/fragment/useSource)、Options/Context(若 P1 未完成)、DeferredCompletionFailureHandler、Preview、Source.Feature | S1 是 ClassReader/ClassFinder 的公共依赖;P4 完整化 |
| S2 | code 最小骨架(P3 前移):Kinds/Kind、Symbol 家族(Symbol/ClassSymbol/MethodSymbol/VarSymbol/ParamSymbol/TypeSymbol/PackageSymbol/ModuleSymbol/RecordComponent)、Scope 家族(Scope/WriteableScope/ErrorScope)、Type 家族(Type/ClassType/ArrayType/MethodType/TypeVar/ForAll/WildcardType)、Attribute 家族、Directive 家族、Symtab(预定义符号/类型 + enterClass/enterPackage/enterModule)、Types 最小 API(erasure/isSameTypes/subst/substBounds/setBounds/...)、TargetType/TypeAnnotationPosition、CompletionFailure/BadClassFile | 这是 ClassReader 的主要前置;建议先"数据结构"后"语义",与 P3 合并推进 |
| S3 | jvm/PoolReader -> jvm/ClassReader(基础:版本/池/类/字段/方法/签名/基础属性)-> ClassReader 属性完整(inner/enclosing/module/record/sealed)-> 注解/type annotations/参数名 | 按 3 个可验证里程碑拆分;每步可跑 jmod/javap 对照 |
| S4 | file 数据源:JavaFileManager/Locations/BaseFileManager/PathFileObject 子集 + JavaFileObject 实现(jimage/jmod/zip/目录/ct.sym)+ ClassFinder(查找/补全/BadClassFile) | jimage/jmod/zip crate 已就绪;ClassFinder 完成模块/包/类查找;ct.sym 见 P8 |
| S5 | 验证:jmod/jimage/javap 对照、坏 class 诊断、差分基座冒烟 | 见测试策略 |

说明:
- jimage/jmod/zip 三个 crate 与诊断访问器(P0)已完成,不阻塞。
- ClassFinder 完整查找策略(模块路径/源路径/class path 优先级/preferredFileObject)
  属 P2 文件层,可与 ClassReader 并行;本 ADR 只定义其与 ClassReader 的边界
  (ClassFinder 负责数据源选择与 completion,ClassReader 只解析字节)。
- 注解/type annotations 建议后置:P2a 基础读取即可支撑 javap 签名对照,先把主链路
  (版本/池/类/字段/方法/签名/基本属性)打通并验证。

### 11. 与 P3/P4 的边界与风险

- ClassReader 输出依赖 P3 的 Symbol/Type/Types/Symtab/Scope/Attribute/Directive 骨架;
  建议在 P2 中把 P3 的"数据结构 + 基础工具"前移(S2),语义分析(Attr/Resolve)仍 P4。
- Log/JCDiagnostic 在 P2 需要最小实现(error/warning/fragment),完整格式化/本地化/
  诊断输出在 P4;但 ClassReader 的诊断 key 与 args 必须从 P2 起正确(用生成访问器)。
- 数据源:class 文件读取只用 JavaFileObject 流;jimage/jmod/zip/ct.sym 的容器差异在
  file 层,避免 ClassReader 分支。
- 风险 1:ClassType 惰性 enclosing 计算在 arena+ID 模型下需要明确的惰性字段约定
  (ADR 0001 规则 5),实现时用字段 + 方法而非匿名子类。
- 风险 2:签名解析的 typevar 两遍处理/missing typevar 延迟替换/旧 GJC 兼容必须严格
  复刻,建议以单元测试逐 token 覆盖。
- 风险 3:注解/type annotations(约 1200 行)与 Annotate 队列时序紧密,建议放到
  ClassReader 第二阶段实现,并用 Annotate 的 block/flush 时序测试保证一致。
- 风险 4:ct.sym 是 zipfs 多版本布局,JRTIndex 的补充 flags 在 P8/P9 才完整;
  P2 内先定义接口与版本选择逻辑,不阻塞主链路。

## 审核裁决记录(Lead 审核通过时提出,已接受并落实到实施约束)

### 裁决
1. **S2 前移:接受(硬约束)**:前移的是"最小 API 范围",不是"简化实现"。
   - 骨架中每个已移植的类型/方法必须按上游完整语义 1:1 移植(字段、惰性、错误路径
     全部在内);只允许暂时不移植 ClassReader 用不到的方法;
   - 后续补全时必须是同一语义(不得重写、不得返回假数据);
   - 未完成清单登记到 docs/STATUS.md(由 Lead 维护)。
2. **注解/type-annotations 分 P2a(基础读取)/P2b(注解/type-annotations/参数名/
   module/record/sealed):接受**,但 P2b 必须在 P2 阶段内完成(最迟 P3 结束前),不得
   拖到 P9;没有 P2b 就没有完全对齐。
3. **ct.sym/JRTIndex 补充 flags 归 P8/P9:接受**;P2 定义接口 + 版本选择逻辑,接口
   不得留死路,多 release 是最终范围(P8 必达)。

### 附加要求
- S1 的 JCDiagnostic/Log 必须是完整移植,不允许为 ClassReader 写"简化版诊断";若某
  方法依赖未移植组件,允许暂缺但必须在 STATUS 逐项登记,并在后续同一文件补全
  (不得另写替代)。
- PoolValue 替代 Object[]、ClassType 惰性 enclosing 用字段+方法、错误 key 列表等
  映射写入 ADR 修订 + PORTING_RULES(由 Lead 登记到 PORTING_RULES §9;本 ADR 已列)。
- ADR 增加"P2 分阶段里程碑与验收"(下一节)。

## P2 分阶段里程碑与验收

P2a(基础读取)完成定义:
- ClassReader 可读:魔数/版本(含 preview major/minor 校验与 BigMajorVersion 警告)、
  常量池(全 tag 校验、延迟解析)、class/field/method 基本结构(flags/名称/描述符)、
  super/interfaces、基础属性(Signature/SourceFile/Deprecated/Synthetic/Exceptions/
  InnerClasses/EnclosingMethod/ConstantValue/MethodParameters/LocalVariableTable 参数名)、
  StackMapTable 等未知属性跳过、readCode 跳过;
- 错误路径:illegal.start.of.class.file / wrong.version / bad.class.file /
  unable.to.access.file / class.file.wrong.class / bad.const.pool.* /
  bad.signature / bad.class.signature / undecl.type.var 的诊断 key 与 args 正确;
- 验收测试:单元(PoolReader/签名逐 token/版本/属性分发/参数名)+ jmod java.base
  6508 类全部读取成功(类数/类名/flags/方法签名摘要与 oracle 对照)+ javap 抽样
  逐项对照 + 坏 class 错误 key 对照。

P2b(注解与其余属性)完成定义:
- 注解:RuntimeVisible/Invisible[Parameter]Annotations/AnnotationDefault/Annotation/
  Bridge/Enum/Varargs 读取为 Proxy 并由 Annotate 队列延迟完成(AnnotationCompleter/
  TypeAnnotationCompleter/AnnotationDefaultCompleter/CompleterDeproxy),
  @Target/@Repeatable/@Deprecated/@PreviewFeature/@ValueBased/@Profile/@Proprietary
  标志处理;
- type annotations:RuntimeVisible/InvisibleTypeAnnotations + TypeAnnotationPosition/
  TargetType/TypePath + addTypeAnnotationsToSymbol/结构映射(符号/类型路径绑定);
- module 属性:Module/ModuleResolution(requires/exports/opens/uses/provides +
  校验 + UsesProvidesCompleter);
- record/sealed:Record 属性(RecordComponent + accessor)+ PermittedSubclasses;
- 注解/参数注解挂到 ParamSymbol、MethodSymbol.defaultValue、Symbol.annotations;
- 验收测试:单元(注解值 tag 集合/type annotation position/结构映射)+ jmod 全量类
  含注解类对照 javap -v(注解计数/类型注解位置)+ 差分基座对注解处理类编译对照。

P2 整体验收:P2a+P2b 全部完成、jmod/jimage 全量读取无错、javap 对照通过、坏 class
诊断对照通过、差分基座冒烟绿;STATUS 登记未完成方法清单(仅允许 ClassReader 用不到
且已登记的方法)。

## 结果

- 明确 ClassReader 只依赖 JavaFileObject 字节流,数据源(jimage/jmod/zip/ct.sym)全部在
  file 层,复用已完成的 javac-jimage/javac-jmod/javac-zip;
- 常量池采用上游的两遍 + 延迟解析模型,PoolValue 枚举替代 Object[] 缓存;
- Signature 解析逐 token 复刻,类型一律进 TypeArena 并返回 TypeId;
- ClassReader 作为"写入器"与 arena+ID 契约明确(输出 Symbol/Scope/Type/Attribute/
  Directive/Annotate 任务,写入点与上游一致,不引入 RefCell 对象图);
- 错误诊断全部走生成的 Fragments/Errors/Warnings 类型化访问器;
- 测试策略覆盖单元/真实数据源/javap 对照/坏 class/差分基座/多 release;
- 依赖顺序建议把 P3 的符号/类型骨架前移(P2 S2),这是关键路径,请 Lead 审核。
