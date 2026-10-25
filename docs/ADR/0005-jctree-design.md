# ADR 0005:JCTree / tree 层设计

- 状态:已接受(Lead 审核通过;修订 ①-④ 见文末"审核修订记录")
- 基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)
- 相关:ADR 0001(arena + ID)、ADR 0003(ClassReader/P2 骨架前移)、ADR 0004(诊断层)、
  docs/PLAN.md P1、docs/PORTING_RULES.md §12-§14

## 背景

tree 层是 javac 语法树:parser 用 TreeMaker 构造 JCTree 节点,后续阶段(Enter/Attr/
Lower/Gen)读写节点上的符号/类型字段,树同时要满足 com.sun.source API(Tree/TreeVisitor)
的对外开放契约。本 ADR 只覆盖 JCTree 及其配套类的设计与移植规则,不实现代码。

上游文件规模(reference/jdk17u 实际 sparse 检出,982d53c;与任务描述数字略有差异,
以实际检出为准):

| Java 文件 | 行数 | 角色 |
|---|---:|---|
| tree/JCTree.java | 3476 | 节点定义(Tag 枚举 105 值 + 67 个具体节点类 + 8 个抽象类 + Visitor/Factory 接口) |
| tree/TreeMaker.java | 1144 | 节点工厂(implements JCTree.Factory) |
| tree/TreeInfo.java | 1381 | 树结构工具(位置/符号/声明查找/构造器判定等) |
| tree/TreeScanner.java | 406 | 内部扫描器(实现 JCTree.Visitor) |
| tree/TreeTranslator.java | 481 | 扫描并替换(visit 返回 void, translate 返回 JCTree) |
| tree/TreeCopier.java | 620 | 树复制(visit 返回 void, result 保存新树) |
| tree/Pretty.java | 1641 | 树到 Java 源码的打印(核心输出工具) |
| tree/EndPosTable.java | 62 | 结束位置表接口(getEndPos/storeEnd/replaceTree) |
| tree/DCTree.java | 1119 | Doc 树节点 |
| tree/DocTreeMaker.java | 732 | Doc 树工厂 |
| tree/DocPretty.java | 654 | Doc 树打印 |
| tree/DocCommentTable.java | 67 | 文档注释表 |
| parser/JavacParser.java | 5039 | 语法分析(构造树、位置、EndPosTable 实现类) |
| parser/JavaTokenizer.java | 1349 | 词法(含 Unicode 转义) |
| parser/UnicodeReader.java | 636 | 输入字符流(Unicode escape 处理) |
| parser/Tokens.java | 459 | token 定义 |
| parser/DocCommentParser.java | 1486 | 文档注释解析 |
| parser/Scanner.java | 137 | token 扫描器 |
| parser/Lexer.java | 87 | token 流缓冲 |
| parser/Parser.java、ParserFactory.java、ScannerFactory.java、JavadocTokenizer.java、LazyDocCommentTable.java、ReferenceParser.java、TextBlockSupport.java | 65/95/94/449/96/216/101 | parser 辅助 |

结构事实(上游):

1. JCTree 公共头:pos(int 位置编码)、type(Type 引用,属性阶段写入)、
   getEndPosition(EndPosTable)(委托 TreeInfo.getEndPos);toString 用 Pretty 打印
   (JCTree 中 new Pretty(s, false).printExpr/printStat)。
2. Tag 枚举实测 105 个值(JDK 17.0.19 内部 API 实测:JCTree.Tag.values().length == 105;含运算符细分:POS/NEG/NOT/COMPL/
   PREINC 等、OR/AND/BITOR 等、BITOR_ASG 等;另有内部 tag TYPEBOUNDKIND/LETEXPR)。
3. 67 个具体节点类 + 8 个抽象节点类(JCCompilationUnit/JCPackageDecl/JCImport/JCClassDecl/JCMethodDecl/
   JCVariableDecl/JCBlock/JCDoWhileLoop/... 表达式家族/类型家族/
   JCModuleDecl/JCRequires/JCExports/JCOpens/JCProvides/JCUses/JCErroneous 等);
   JCCompilationUnit 含 defs/sourcefile/docComments/lineMap?/endPositions 等字段;
   7 个节点带 Symbol 字段:JCModuleDecl.sym、JCClassDecl.sym、JCMethodDecl.sym、
   JCVariableDecl.sym、JCFieldAccess.sym、JCMemberReference.sym、JCIdent.sym。
4. Visitor:JCTree.Visitor 每个节点 visitXxx(JCTree) 默认 visitTree(Assert.error);
   accept(TreeVisitor) 逐节点分派。
5. Factory:JCTree.Factory 接口(工厂方法),TreeMaker implements JCTree.Factory;
   TreeMaker 字段 pos/toplevel/names/types/syms,方法 at(pos)/at(DiagnosticPosition) +
   约 80 个 create 方法(TopLevel/PackageDecl/ClassDef/MethodDef/VarDef/.../Ident(Symbol)/
   Select(Symbol)/ClassLiteral(Type) 等,其中需要 Names/Types/Symtab 的方法依赖 P3)。
6. EndPosTable:接口 3 方法(getEndPos(JCTree)/storeEnd(JCTree,int)/
   replaceTree(JCTree,JCTree));实现类 AbstractEndPosTable/SimpleEndPosTable/
   EmptyEndPosTable 在 JavacParser 内部(Simple 用 Map<JCTree,Integer>);parser 以
   keepEndPositions(来自 -Xjcov/内部调用)决定 Simple 或 Empty。
7. TreeInfo:位置(getStartPos/getEndPos/firstStatPos/endPos)、结构判定(isConstructor/
   isMultiCatch/isExpressionStatement/isStatement/skipParens/...)、需要符号/类型的工具
   (symbolFor/declarationFor/typeIn/isStaticSelector/firstConstructorCall 等)、注释工具
   (getCommentText/getCommentTree,依赖 DCTree)。

当前 rust-javac 现状:

- com.sun.source 接口层已完成(crates/javac-source:Tree trait 1334 行含 get_kind/
  Kind 115 个/71 个 as_xxx 对象转换方法;TreeVisitor 727 行;70 个节点 trait 文件;
  doctree/util 包已移植);
- javac-core:tree/ 只有 mod.rs(空),parser/ 空,code/ 只有 Flags/Kinds 等 5 个小文件;
- ADR 0001 已定 arena+ID 模型;ADR 0003 已定 ClassReader 所需的 code 骨架前移(S2)。

## 决策

### 1. JCTree 的 Rust 表达(外层 struct + enum kind + arena)

    节点公共头 + 节点特有字段(每个节点一个 TreeKind 变体):
    pub struct JCTree {
        pub tag: Tag,           // javac 内部 Tag(105 个,运算符细分)
        pub pos: i32,           // 位置编码(util::position;NOPOS = -1)
        pub ty: Option<TypeId>, // 上游 JCTree.type(属性阶段写入,null 用 None)
        pub kind: TreeKind,     // 节点特有字段(1:1 上游节点类字段)
    }
    pub enum TreeKind {
        PackageDecl { annotations: List<TreeId>, pid: TreeId, ... },
        ClassDef { mods: TreeId, name: Name, typarams: List<TreeId>,
                   extending: Option<TreeId>, implementing: List<TreeId>,
                   permitting: List<TreeId>, defs: List<TreeId>, ... },
        // ... 67 个变体(首批实现 63 个;JCAnnotation/JCExports/JCOpens/JCRequires 待 code/Attribute/Directive),字段与上游 JCTree 节点类逐一对应
    }
    pub struct TreeArena { trees: Vec<JCTree> }
    pub struct TreeId(u32);

规则:

- 子节点引用一律 TreeId(javac 的 null 用 Option<TreeId>);节点列表用
  util::list::List<TreeId>(与上游 List<JCTree> 对应;Rc 链表共享 tail,O(1) clone,
  便于遍历时避免借用冲突);
- 标量:Name/JavaString/boolean/int/long(按上游字段类型);运算符节点复用 Tag
  (JCOperatorExpression 的 opcode 是 Tag,不是单独枚举);
- 7 个带 Symbol 字段的节点用 Option<SymbolId>(parser 阶段通常为 None,Enter(P3) 写入);
  其它符号/类型字段按上游字段映射;
- 抽象节点类(JCExpression/JCStatement/JCPolyExpression/JCFunctionalExpression/
  JCOperatorExpression/JCPattern/JCDirective)没有自己的变体:其字段并入具体变体
  (poly_kind/target/operator);实测 JCTree 有 67 个具体节点类(首批 63 个变体,
  JCAnnotation/JCExports/JCOpens/JCRequires 依赖 code/Attribute 与 code/Directive,
  待 code 骨架后补齐);
- JCCompilationUnit 的 sourcefile 用 Option<Rc<dyn JavaFileObject>>(javac-tools 契约),
  docComments/endPositions 按上游映射(EndPosTable 见第 3 节);
- arena 契约(ADR 0001):只增不减,ctx.tree(id)/ctx.tree_mut(id)/ctx.alloc_tree(...);
  分配 API 放在 CompilerContext 上,ID 有效性与 debug 断言按 ADR 0001;
- Display:JCTree 的 toString 用 Pretty 打印(JCTree 上游行为),Rust 用
  impl Display for JCTree(无编译器上下文,与上游一致);
- Tag 与 com.sun.source Kind 是两套表示:Tag 是内部 tag(105,运算符细分),
  Kind 是 API 视图(115,javac-source 已定)。提供 tag_to_kind(Tag) -> Kind 映射
  (运算符 tag 归并到 Unary/Binary/CompoundAssignment 等;TYPEBOUNDKIND/LETEXPR 是内部
  节点,上游 getKind 抛 AssertionError,映射函数 panic 等价);实现时用穷尽 match
  保证 105 个 Tag 全覆盖,并以测试断言 115 个 Kind 都有对应 Tag(覆盖矩阵测试)。

### 2. 与 com.sun.source Tree trait 的对接

- P1 内部不实现 javac-source 的节点 getter trait:该 trait 的 getter 返回
  Rc<dyn XxxTree>(共享所有权),arena+ID 模型没有 Rc<dyn Tree>,且 getter 无 ctx 参数
  无法解析子节点 ID。P1 提供内部访问 API:TreeKind 变体 match + 条件访问器
  (as_class_decl/as_method_decl/... 等价于上游 instanceof+cast,命名 as_xxx 的内部版本,
  返回 Option<&XxxData>)。
- get_kind(115)+ as_xxx(71):get_kind 由 tag_to_kind 实现(P1 可用);
  javac-source 的 71 个 as_xxx 在 P6 视图层实现(见下)。
- accept/dispatch:javac-source 的 accept(TreeVisitor) 带 where Self: Sized,不能用于
  dyn;P1 内部 Visitor 用 trait(JCTreeVisitor):
      fn visit_class_def(&mut self, ctx: &mut CompilerContext, id: TreeId) { self.visit_tree(ctx, id) }
      fn visit_tree(&mut self, ctx: &mut CompilerContext, id: TreeId);
  (默认 visitXxx 委托 visitTree,上游语义;结果字段按上游 void visitor 处理),
  P1 的 Scanner/Translator/Copier/Pretty 实现该 trait。
- P6 的 com.sun.source 视图适配契约(预登记,不在 P1 实现):
  - 定义 ApiTree:持有 arena 访问句柄 + TreeId,实现 javac-source 的 Tree + 全部节点
    trait;getters 按 TreeKind 取字段、子节点返回 Rc<ApiTree>;kind 不匹配时 panic
    (Java ClassCastException 语义,仅会通过 as_xxx 走到正确 trait);
  - accept(TreeVisitor) 的 dispatch 由 get_kind + match 实现(与 javac-source
    PORTING_RULES 的 util 层 dispatch 一致);
  - arena 访问句柄用 Rc<RefCell<CompilerContext 视图>> 或一次性树转换;若用
    Rc<RefCell>,按 ADR 0001 白名单登记(P6 决定具体方案);
  - P1 不改 javac-source 的契约;P1 的 as_xxx 内部访问器与 P6 视图共用 kind 匹配逻辑。

### 3. 位置 / EndPosTable

- 位置:pos 为 int 编码(util::position 的编码/NOPOS=-1;已有 LineMap/Position 实现);
  parser 通过 TreeMaker.at(pos) 设置;TreeInfo.getStartPos 等按上游算法计算。
- EndPosTable(trait,1:1 tree/EndPosTable.java):
      fn get_end_pos(&self, tree: TreeId) -> i32;
      fn store_end(&mut self, tree: TreeId, endpos: i32);
      fn replace_tree(&mut self, old_tree: TreeId, new_tree: TreeId) -> i32;
  实现类随 parser 移植(AbstractEndPosTable/SimpleEndPosTable/EmptyEndPosTable,
  Simple 用 HashMap<TreeId,i32>);JCCompilationUnit.end_positions 用
  Option<Rc<RefCell<dyn EndPosTable>>> 或 enum(Simple/Empty)持有;keepEndPositions
  (-Xjcov/内部调用)决定实现;TreeInfo.getEndPos 用该表(无表时 NOPOS)。
- 与 JCDiagnostic/DiagnosticPosition 的对接在 D5/P4(ADR 0004):JCTree 将实现
  DiagnosticPosition 等价接口(getStartPosition/getPreferredPosition/getEndPosition),
  P1 先提供内部位置访问函数,JCDiagnostic 落地后接真实接口。

### 4. type/sym 写入与借用模型

- 写入点与上游一致:parser 不写 sym/type(上游 parser 只做结构构造);Enter(P3)写 sym;
  Attr(P4)写 ty;节点字段为 Option<SymbolId>/Option<TypeId>,写入函数如
  ctx.tree_mut(id).ty = Some(type_id)(写入点集中,便于审计)。
- Visitor/Scanner 签名带 ctx:
      fn visit_xxx(&mut self, ctx: &mut CompilerContext, id: TreeId);
  遍历子节点前先 clone List<TreeId>(O(1) Rc clone),再递归,避免同时持有 &JCTree 的
  不可变借用与 &mut CompilerContext 的可变借用;需要读节点字段+递归时采用"先读后写"
  两阶段(读取字段到局部变量,再递归)。
- TreeInfo 的函数按依赖分层:纯结构函数(只读树)与符号/类型相关函数(需要 ctx);
  符号相关函数依赖 P3 的 Symbol/Types。
- TreeMaker:
      pub struct TreeMaker { pub pos: i32, pub toplevel: Option<TreeId> }
  (上游 names/types/syms 字段由 CompilerContext 提供,不重复存储;登记映射);
  方法:at(pos)->&mut Self、at_diagnostic_position(...);
  make_top_level/make_class_def/... (约 80 个工厂方法,1:1 上游,返回 TreeId);
  需要 Names/Types/Symtab 的工厂(Ident(Symbol)/Select(Symbol)/QualIdent/ClassLiteral 等)
  接收 ctx;JCTree.Factory 接口保留为 Rust trait(TreeFactory,1:1),parser 使用具体
  TreeMaker。
- 借用不变量:arena 访问器不返回跨可变操作的引用;树节点上的子 ID 读取后再分配/修改,
  避免同时悬垂借用;编译期 arena 只增不减(ADR 0001)。

### 5. TreeInfo/Scanner/Translator/Copier/Pretty/Doc 移植规则

| 上游 | Rust | 规则/注意 |
|---|---|---|
| TreeInfo | tree/tree_info.rs | 纯函数 1:1;符号/类型相关函数随 P3 在同一个文件补全(STATUS 登记,不写替代);注释工具(DCTree)随 Doc 树 |
| TreeScanner | tree/tree_scanner.rs | 内部扫描器(与 com.sun.source.util.TreeScanner API 层区分);实现 JCTreeVisitor;scan(Option<TreeId>) + visit 默认遍历 |
| TreeTranslator | tree/tree_translator.rs | visit 返回 void + result/translate 语义:translate(&mut self, ctx, id)->TreeId;字段写回与上游一致 |
| TreeCopier | tree/tree_copier.rs | 复制到新 TreeId(arena 追加);浅拷贝字段 + 递归复制;上游 result 字段语义保留 |
| Pretty | tree/pretty.rs | 输出 String(上游 Writer);sourceOutput 标志、缩进/优先级/注释;实现 JCTreeVisitor;print_unit/print_stat/print_expr/print_type 等 |
| DCTree/DocTreeMaker/DocPretty/DocCommentTable/LazyDocCommentTable/DocCommentParser/JavadocTokenizer | tree/dc_tree.rs 等(与 parser 协作) | Doc 树复用 javac-source 的 doctree 接口;实现与 Pretty 相同的对照测试 |

说明:javac 内部存在两套 Scanner/Visitor:tree.TreeScanner(JCTree 内部,本 ADR 范围)与
com.sun.source.util.TreeScanner(API 层,javac-source 已移植),二者不要混淆。

### 6. 测试策略

1. Pretty 输出与 javac 对照(关键验收):
   - oracle 方式:开发期 Java 程序(需要 --add-exports jdk.compiler/com.sun.tools.javac.tree
     等)用 JavacTool/JavacTask.parse() 得到 JCCompilationUnit,再用
     new Pretty(writer, sourceOutput).printUnit/printStat/printExpr 打印,生成对照数据;
     数据入库,运行期零 JVM 依赖;
   - 注意:javac 的 -Xprint 不是 Pretty 输出(它走 PrintingProcessor,打印 API 摘要,
     实测输出形如类体摘要),不能用作 Pretty 对照;-s(sourceOutput)是 Pretty(true) 的
     行为,仅作辅助;
   - 语料:test/langtools/tools/javac 的 compile-only 用例(task-11 索引/基线,含
     语法/表达式/泛型/注解/lambda/record/sealed/module 用例)+ 语法错误用例
     (Erroneous);对照用 javac-tests 的 normalize/compare(CRLF/路径/编码归一化);
   - 覆盖度:按 Kind/节点类型统计语料覆盖,未覆盖节点补充单元用例;P1 结束要求
     语料 parse + Pretty 输出 100% 与 oracle 逐字一致。
2. 结构化单测:
   - Tag->Kind 映射:105 个 Tag 全覆盖 + 115 个 Kind 全部有对应(覆盖矩阵测试);
   - JCTree arena:分配/访问/位置/Display;TreeKind 字段与上游节点类字段对照;
   - EndPosTable:Simple/Empty 的 getEndPos/storeEnd/replaceTree;keepEndPositions 语义;
   - TreeInfo:纯结构函数(位置/endPos/firstStatPos/skipParens/isConstructor 等);
   - TreeScanner:访问顺序与默认遍历;TreeTranslator:替换写回;TreeCopier:深拷贝
     (Pretty 输出相同 + 原树不变);
   - 语法错误恢复:Erroneous 结构/位置(与 javac 对照数据)。
3. Doc 树:DocCommentParser/DocPretty 与开发期 Java oracle 对照(同 Pretty 方式)。
4. 差分基座(javac-tests):parse-only 阶段(-XDshould-stop.at=PARSE 等价)对语料
   parse+Pretty 与 javac 对照,作为 P1 阶段验收。

### 7. P1 起步顺序与依赖清单

依赖(前置条件):
- util:List/ListBuffer/Position/Names/Name/Convert/Assert/ByteBuffer 等已具备;
- code 最小骨架(P2 S2 前移共享):SymbolId/TypeId 类型与 SymbolKind/Kinds 骨架
  (tree 节点 Option<SymbolId>/Option<TypeId> 只需要 ID 类型;完整语义随 P3 补全,
  同一文件同一语义,不写替代);
- javac-source 接口层已就绪(P6 视图/契约);
- javac-tools 的 JavaFileObject 契约已就绪;
- Flags 已具备。

顺序(每阶段 cargo test 绿、clippy 零诊断):
1. tree/end_pos_table + JCTree:Tag/TreeKind/arena/访问器/as_xxx 内部映射/Display +
   tag_to_kind 映射 + Visitor trait;
2. TreeMaker(工厂,约 80 方法);
3. TreeInfo(纯结构子集;符号/类型相关随 P3 同文件补全);
4. TreeScanner/TreeTranslator/TreeCopier;
5. Pretty(+ 开发期 oracle 对照数据);
6. parser 基础设施:Lexer/Tokens/JavaTokenizer/UnicodeReader/Scanner/ScannerFactory/
   Parser/ParserFactory;
7. JavacParser + EndPosTable 实现类(Abstract/Simple/Empty)+ DocCommentTable 基础;
8. Doc 树族(DCTree/DocTreeMaker/DocPretty/DocCommentTable/LazyDocCommentTable/
   DocCommentParser/JavadocTokenizer);
9. 验证:Pretty 全量对照/语料 parse 基线/差分基座冒烟。

P1 内不依赖 P3/P4 完成的部分:树结构/位置/工厂/扫描/翻译/复制/Pretty/Doc 树;
符号相关(Enter)与类型相关(Attr)的字段写入在 P3/P4,但字段类型与写入 API 在 P1 定好
(Option<SymbolId>/Option<TypeId>),不阻塞。

### 8. 差异/风险登记

- Tag(105)与 Kind(115)双表示:内部节点 TYPEBOUNDKIND/LETEXPR 无对应 Kind(上游
  getKind 抛 AssertionError),映射函数 panic 等价;映射覆盖用穷尽 match 保证;
- P6 视图适配:Rc<dyn Tree> 与 arena+ID 的桥接(ApiTree + 共享 ctx)属 P6 设计,
  可能引入 Rc<RefCell>(ADR 0001 白名单登记),P1 不改 javac-source 契约;
- TreeInfo/其他文件中的符号/类型方法随 P3 补全(STATUS 登记),不写简化替代;
- Pretty 逐字对齐风险:空格/换行/括号优先级/生成注释/Erroneous 结构,要求 oracle
  数据覆盖全节点;Doc 注释打印与 DocCommentTable 时序(parser 保留注释)相关;
- 树节点 sym/type 的可变写入遵循 ADR 0001(写入点与上游一致);
- JCTree.toString/Display 依赖 Pretty(不依赖编译器上下文)。

## 审核修订记录(Lead 审核通过时提出,已接受并落实到实施约束)

- **① P1 内部表示与 P6 API 视图分离:批准。**
  - P6 视图适配必须逐 JCTree 节点 getter 对照(返回类型/可空/顺序),列入 P6 验收;
  - P1 不得写简化 getter;内部遍历/打印/变换顺序与上游一致(Scanner/Translator/
    Copier/Pretty 的 visit 顺序按上游逐行移植)。
- **② Tag/Kind 双表示:批准。**
  - Tag 数量与 Kind 115 用实测值写死断言(Tag 实测 105,见背景;实现时以穷尽 match
    保证 Tag 全覆盖,并以断言把 105/115 写成常量);
  - 内部节点(TYPEBOUNDKIND/LETEXPR 等)无 Kind 的行为用 Java 内部 API 实测确认
    (AssertionError -> Rust panic 等价)并登记到 PORTING_RULES;
  - tag_to_kind 用穷尽 match(不得 catch-all 分支)。
- **③ Pretty oracle:批准。**
  - oracle 生成程序/命令/JDK 版本记录入库:程序放 tools/gen-tree-oracle/(与
    tools/gen-unicode 并列),命令与版本写入 ADR 附录或 README;
  - 对照数据入库,运行期零 JVM;覆盖 Kind 100% + corpus 抽样;
  - -Xprint 不作 oracle 的结论保持(-Xprint 走 PrintingProcessor 打印 API 摘要)。
- **④ P1 顺序 S1-S9:批准。**
  - 每步补完成定义与验证(见下节 S1-S9 完成定义表);
  - ID/arena 契约(ADR 0001)由 Lead 先落地,S1 依赖它;与 ADR 0003 S2 共享清单
    (SymbolId/TypeId/Kinds 骨架前移项以 ADR 0003 S2 为准,不重复定义)。

### P1 各步完成定义与验证(修订 ④ 落实)

| 步骤 | 完成定义 | 验证 |
|---|---|---|
| S1 JCTree/EndPosTable | JCTree 公共头+TreeKind 67 变体(首批 63,4 待 code 骨架)+arena+Tag/Visitor/EndPosTable trait+as_xxx 内部访问器+Display+tag_to_kind 穷尽 match | cargo test 绿 + Tag/Kind 覆盖矩阵测试 + EndPosTable 单测 + Display 与 Pretty 对照(先最小树) |
| S2 TreeMaker | 约 80 工厂方法 1:1(依赖 ctx 的工厂签名带 ctx)+ at/at_diagnostic_position | 工厂构造的树与上游字段一致单测;Pretty 对照(工厂构造最小树) |
| S3 TreeInfo | 纯结构子集 1:1(位置/endPos/firstStatPos/skipParens/isConstructor 等);符号/类型方法 TODO(P3,同文件补全) | 纯结构函数单测 + 编译期 TODO 清单与 STATUS 登记 |
| S4 Scanner/Translator/Copier | JCTreeVisitor 实现;scan 顺序/translate 写回/copy 深拷贝与上游一致 | 访问顺序断言;翻译后 Pretty 对照;复制后 Pretty 相同且原树不变 |
| S5 Pretty | print_unit/print_stat/print_expr/print_type 1:1;oracle 数据入库 | Pretty 与 oracle 逐字对照(覆盖 Kind 100% + corpus 抽样) |
| S6 parser 基础设施 | Lexer/Tokens/JavaTokenizer/UnicodeReader/Scanner/ScannerFactory/Parser/ParserFactory 1:1 | token 流对照(oracle 数据)、Unicode escape/文本块单测 |
| S7 JavacParser+EndPosTable 实现 | parse 全语法 1:1;Abstract/Simple/EmptyEndPosTable;docComments 基础;-XDshould-stop.at=PARSE 等价 | 语料 parse+Pretty 与 javac 对照;语法错误树/位置对照;keepEndPositions(-Xjcov) 语义 |
| S8 Doc 树族 | DCTree/DocTreeMaker/DocPretty/DocCommentTable/LazyDocCommentTable/DocCommentParser/JavadocTokenizer 1:1 | DocPretty 与 oracle 对照;doc comment 保留/懒解析语义 |
| S9 验证 | 全语料 parse+Pretty 100% 对照;差分基座冒烟;STATUS 更新 | cargo test 全绿、clippy 零诊断、差分报告 |

## 结果

- JCTree 采用 arena+ID + 外层 struct(JCTree 公共头)+ TreeKind enum(节点字段),
  与 ADR 0001 一致;Tag/Kind 双表示通过映射函数对接 com.sun.source;
- P1 内部 Visitor 与 P6 API 视图分离,javac-source 契约不变;
- EndPosTable/位置/type-sym 写入与借用模型明确;
- TreeInfo/Scanner/Translator/Copier/Pretty/Doc 全部给出 1:1 移植规则;
- 测试以 Pretty 与 javac 逐字对照为核心验收,附结构化单测与差分基座;
- P1 顺序与依赖清单明确,符号/类型字段类型在 P1 定好、写入随 P3/P4 补全。
