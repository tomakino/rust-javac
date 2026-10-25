# ADR 0006:parser 层设计

- 状态:已接受(Lead 审核通过;修订 ①-⑤ 见文末"审核修订记录")
- 基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)
- 相关:ADR 0005(JCTree/tree 层)、ADR 0002/0004(诊断资源与诊断层)、docs/PLAN.md P1

## 背景

parser 层把 Java 源文件的字符流变成 JCTree 语法树:词法分析器(UnicodeReader +
JavaTokenizer + Scanner/Lexer)产出 token 流,手写递归下降 parser(JavacParser)按
JLS 语法构造树,并用 ErrorRecoveryAction/skip 做语法错误恢复;doc comment 由
JavadocTokenizer 收集、LazyDocCommentTable 延迟解析。本 ADR 只做设计,不实现代码。

上游文件规模(reference/jdk17u 实际 sparse 检出,982d53c):

| Java 文件 | 行数 | 角色 |
|---|---:|---|
| parser/JavacParser.java | 5039 | 手写递归下降 + 运算符优先级(infixPrecedenceLevels=10);实现 Parser;含 EndPosTable 实现类 |
| parser/JavaTokenizer.java | 1349 | 词法:Unicode 转义/数字/字符串/文本块/注释/运算符;extends UnicodeReader |
| parser/UnicodeReader.java | 636 | 字符流 + Unicode escape 解码(eligible backslash 规则) |
| parser/Tokens.java | 459 | TokenKind(实测 114 值)/Token 类层次/Comment/keywords map |
| parser/DocCommentParser.java | 1486 | doc comment 解析为 DCTree |
| parser/JavadocTokenizer.java | 449 | doc comment token(提取正文/offsetMap/@deprecated) |
| parser/ReferenceParser.java | 216 | doc 引用(@see/@link)解析 |
| parser/TextBlockSupport.java | 101 | 文本块 lint 检查(INCONSISTENT/TRAILING) |
| parser/Scanner.java | 137 | Lexer 实现(token/prevToken/前瞻缓冲/split) |
| parser/Lexer.java | 87 | 词法接口 |
| parser/Parser.java | 65 | Parser 接口(parseCompilationUnit/parseExpression/parseStatement/parseType) |
| parser/ParserFactory.java | 95 | parser 工厂与组件 |
| parser/ScannerFactory.java | 94 | scanner 工厂(keepDocComments -> JavadocTokenizer) |
| parser/LazyDocCommentTable.java | 96 | DocCommentTable 的延迟实现 |
| tree/DocCommentTable.java | 67 | doc comment 表接口(ADR 0005 的 tree 层) |

关键结构(上游):

1. Tokens:TokenKind 枚举实测 114 个值(EOF/ERROR/IDENTIFIER/关键字/字面量/运算符/
   MONKEYS_AT/CUSTOM),每项可带 name(关键字/运算符文本)与内部 Tag
   (DEFAULT/NAMED/STRING/NUMERIC);TokenKind implements Formattable(toString ->
   token 资源键)与 Predicate<TokenKind>;Token 基类(kind/pos/endPos/comments)+
   NamedToken(name)/StringToken(stringVal)/NumericToken(radix)子类;DUMMY;split;
   comment(CommentStyle)/deprecatedFlag;Comment 接口(getText/getSourcePos/
   getStyle/isDeprecated)+ CommentStyle(LINE/BLOCK/JAVADOC);Tokens 组件维护
   keywords map(name -> TokenKind)与 lookupKind。
2. UnicodeReader:char[] + position/width/character/codepoint/wasBackslash/
   wasUnicodeEscape;nextUnicodeInputCharacter 处理 eligible backslash 规则;
   unicodeEscape 支持多个 u 与 4 位十六进制,非法转义报 IllegalUnicodeEsc 后跳过;
   nextCodePoint 处理代理对;position/reset/get/next/accept/skip/is/isOneOf/
   inRange/digit/getRawCharacters 等。
3. JavaTokenizer extends UnicodeReader:字段 tk/radix/name/errPos/isTextBlock/
   hasEscapeSequences/sb/source/preview/log/tokens/names/lint;readToken 主循环
   (空白/行终止符/标识符/数字(十六进制/二进制/八进制/十进制、下划线检查)/
   字符/字符串/文本块/运算符/EOF);scanIdent/scanNumber/scanDigits/
   scanHexExponentAndSuffix/scanString/scanLitChar/scanOperator(最长匹配)/
   isSpecial;processWhiteSpace/processLineTerminator(子类覆盖);lexError/lexWarning
   用生成的 Errors/Warnings 访问器与 JCDiagnostic flags。
4. Scanner implements Lexer:token/prevToken/savedTokens(前瞻缓冲)/nextToken/split/
   errPos/getLineMap;ScannerFactory.newScanner(CharSequence/char[], keepDocComments)
   -> JavadocTokenizer 或 JavaTokenizer。
5. JavadocTokenizer extends JavaTokenizer:processComment(pos,endPos,style) 用
   getRawCharacters 生成 JavadocComment(BasicComment 子类:提取 doc 正文、
   offsetMap 回映源位置、@deprecated Pattern 检测);getLineMap 覆盖。
6. ParserFactory:组件 F(TreeMaker)/docTreeMaker/log/names/tokens/source/preview/
   options/scannerFactory/locale;newParser(input, keepDocComments, keepEndPos,
   keepLineMap, parseModuleInfo) -> JavacParser;调用方:JavaCompiler.parse
   (keepComments = keepComments()||sourceOutput,-s 时保留注释;genEndPos 由
   -Xjcov 等决定)、JavacTaskImpl(全 false)、ReferenceParser(全 false)。
7. JavacParser:字段 S(Lexer)/F(TreeMaker)/log/source/preview/names/
   endPosTable(AbstractEndPosTable)/typeAnnotationsPushedBack/
   permitTypeAnnotationsPushBack/docComments/keepDocComments/mode/lastmode/
   token/errorPos/count/parseModuleInfo;核心流程 parseCompilationUnit(首 token/
   package/import/typeDeclaration/module 声明/错误恢复 skip/attach docComment/
   toplevel.docComments/lineMap/endPosTable.setParser(null)/toplevel.endPositions);
   语法(typeDeclaration/class/record/interface/enum/annotation 声明、method、
   statement、expression(term/term3)、type、pattern(instanceof/switch)、lambda、
   memberReference、module 声明);位置(to/toP = storeEnd 当前/前一个 token 结束位置;
   getStartPos/getEndPos 代理 TreeInfo);错误恢复(ErrorRecoveryAction/
   BasicErrorRecoveryAction、RECOVERY_THRESHOLD=50、skip 四个 stop 条件、
   syntaxError -> JCErroneous + reportSyntaxError + setErrorEndPos + toP、
   reportSyntaxError 按 RECOVERY_THRESHOLD 去重)。
8. EndPosTable 实现类(在 JavacParser 内):AbstractEndPosTable(parser 引用/
   errorEndPos/setErrorEndPos/setParser/to/toP 抽象)、SimpleEndPosTable
   (IntHashTable 存储;storeEnd 取 errorEndPos 与 endpos 较大者;getEndPos/
   replaceTree)、EmptyEndPosTable(全空实现);keepEndPositions 由 genEndPos
   (-Xjcov 等)决定。
9. Doc:JavacParser 用 token.comment(JAVADOC) 取注释并 attach 到树(keepDocComments
   时 putComment);JCCompilationUnit.docComments = LazyDocCommentTable;
   getCommentTree 延迟 DocCommentParser.parse()(DCTree);DocCommentTable 接口
   (hasComment/getComment/getCommentText/getCommentTree/putComment);
   ReferenceParser 解析 @see/@link 引用(内部用 ParserFactory.newParser 解析片段);
   TextBlockSupport 做文本块 lint 检查。

当前 rust-javac 现状:parser/ 与 tree/ 都还是空模块;javac-core code/ 只有
Flags/Kinds 等小文件;ADR 0005 已定 JCTree 的 arena+ID 表示与 P1 顺序;javac-source
的 com.sun.source 接口层(含 doctree)已完成;task-19/23 生成的诊断与 token 资源表
(compiler.misc.token.*)可用;util 已有 List/ListBuffer/Position/Names/IntHashTable/
LayoutCharacters/Convert/Assert 等。Source/Preview/Lint 尚未移植(parser 需要,见依赖)。

## 决策

### 1. 模块映射

| Java | Rust |
|---|---|
| parser/Tokens.java | crates/javac-core/src/parser/tokens.rs |
| parser/UnicodeReader.java | crates/javac-core/src/parser/unicode_reader.rs |
| parser/JavaTokenizer.java | crates/javac-core/src/parser/java_tokenizer.rs |
| parser/JavadocTokenizer.java | crates/javac-core/src/parser/javadoc_tokenizer.rs |
| parser/Scanner.java | crates/javac-core/src/parser/scanner.rs |
| parser/Lexer.java | crates/javac-core/src/parser/lexer.rs |
| parser/Parser.java | crates/javac-core/src/parser/parser.rs |
| parser/ParserFactory.java | crates/javac-core/src/parser/parser_factory.rs |
| parser/ScannerFactory.java | crates/javac-core/src/parser/scanner_factory.rs |
| parser/JavacParser.java | crates/javac-core/src/parser/javac_parser.rs(含 EndPosTable 实现类) |
| parser/DocCommentParser.java | crates/javac-core/src/parser/doc_comment_parser.rs |
| parser/ReferenceParser.java | crates/javac-core/src/parser/reference_parser.rs |
| parser/TextBlockSupport.java | crates/javac-core/src/parser/text_block_support.rs |
| parser/LazyDocCommentTable.java | crates/javac-core/src/parser/lazy_doc_comment_table.rs |
| tree/DocCommentTable.java | tree/doc_comment_table.rs(ADR 0005) |

1:1 规则同上(来源锚点/英文 Javadoc/snake_case/控制流照抄);Java 重载 -> Rust 不同名
方法(PORTING_RULES §3);Java char/String -> javac-strings(u16/JavaString);long/int
按上游;接口 -> trait;匿名类/内部类 -> 同文件的 struct/enum。

### 2. Token 模型

- TokenKind:实测 114 个值 -> Rust enum TokenKind,按 PORTING_RULES §3 保留 Java 原名
  (#[allow(non_camel_case_types)]),114 值用全量测试断言;上游每项的 name(关键字/
  运算符文本)与内部 Tag(DEFAULT/NAMED/STRING/NUMERIC)用 impl 方法
  kind_name()/kind_tag() 表达;
  implements Formattable(getKind = "Token"; to_string(locale,messages) 用
  JavacMessages 与 compiler.misc.token.* 资源键,已由生成表提供)与
  Predicate(test = 相等)。
- Token:struct Token { kind: TokenKind, pos: i32, end_pos: i32,
  comments: List<Rc<dyn Comment>>, value: TokenValue };
  enum TokenValue { None, Named(Name), String(JavaString), Numeric(JavaString, i32) }
  (替代上游 NamedToken/StringToken/NumericToken 类层次;上游 String -> JavaString)。
  DUMMY 常量;split(由 kind.name 拆两个 token,AssertionError -> panic 等价);
  comment(CommentStyle)/deprecated_flag/get_comments 方法 1:1。
- Comment:trait Comment { get_text/get_source_pos/get_style/is_deprecated };
  CommentStyle 枚举(LINE/BLOCK/JAVADOC);BasicComment(JavaTokenizer 内,
  PositionTrackingReader 子类)与 JavadocComment(JavadocTokenizer 内,
  正文提取 + OffsetMap + @deprecated Pattern)分别实现。
- Tokens 组件:Context 键(TypeId 模型)、keywords: HashMap<JavaString, TokenKind>
  (构造时对所有 name 非空的 TokenKind 注册 names.fromString)、lookupKind(Name/
  JavaString)。

### 3. 词法层

- UnicodeReader:输入为 JavaString/Vec<u16>(UTF-16 code units,与 javac-strings
  一致;JavaFileObject 内容读取后为 JavaString) + length;字段 position/width/
  character(u16)/codepoint(u32)/was_backslash/was_unicode_escape;方法 1:1:
  next_code_unit/next_unicode_input_character(eligible backslash 规则)/
  next_code_point(代理对)/unicode_escape(多个 u + 4 位十六进制;非法 ->
  Errors.IllegalUnicodeEsc 后跳过继续)/position/reset/get/get_codepoint/
  is_surrogate/is_ascii/next/is/is_one_of/in_range/accept/accept_one_of/skip/
  skip_whitespace/skip_to_eoln/accept_str/digit/get_raw_characters;EOI 用
  util::layout_characters(已有)。
- JavaTokenizer:字段与 readToken 主循环 1:1(空白/行终止符/标识符/数字(0x/0b/0/
  十进制+下划线检查)/字符/字符串/文本块/运算符/EOF);scan_ident/scan_number/
  scan_digits/scan_hex_exponent_and_suffix/scan_string/scan_lit_char/
  scan_operator(最长匹配)/is_special;process_white_space/process_line_terminator
  由子类覆盖;lex_error/lex_warning 用生成的 Errors/Warnings 访问器 +
  JCDiagnostic.DiagnosticFlag/LintCategory;文本块扫描调用 TextBlockSupport 做
  lint 检查(INCONSISTENT/TRAILING)。
- Scanner implements Lexer:token/prev_token/saved_tokens(Vec 前瞻)/
  next_token/split/err_pos/line_map,语义 1:1;ScannerFactory.new_scanner(input,
  keep_doc_comments) -> JavadocTokenizer 或 JavaTokenizer。
- JavadocTokenizer:process_comment -> JavadocComment(get_raw_characters(pos,
  end_pos) 提取 doc 正文 + offsetMap 回映 + DEPRECATED_PATTERN);get_line_map 覆盖。
- Unicode 转义语义(JDK 17):多个 u 前缀、eligible backslash(wasBackslash/
  wasUnicodeEscape)、非法转义报错后跳过;严格按上游状态机移植。

### 4. Parser 层

- Parser 接口 -> Rust trait Parser(parse_compilation_unit/parse_expression/
  parse_statement/parse_type,返回 TreeId;ADR 0005 树表示)。
- ParserFactory:struct 持 Context 访问与组件(F/doc_tree_maker/log/names/
  tokens/source/preview/options/scanner_factory/locale);new_parser(input,
  keep_doc_comments, keep_end_pos, keep_line_map, parse_module_info)。
- JavacParser:struct 字段与上游一一对应;核心流程:
  * parse_compilation_unit(首 token/package/import/typeDeclaration/module 声明/
    错误恢复 skip/attach doc comment/toplevel.doc_comments/lineMap/
    endPosTable.setParser(null)/toplevel.end_positions);
  * 语法构造 1:1(顶层声明、module 声明、import/package、类型声明 class/record/
    interface/enum/annotation、成员、语句、表达式(term 10 级优先级/term3)、类型、
    pattern(instanceof/switch)、lambda、member reference、注解/类型注解
    pushback);
  * 位置:to/toP(storeEnd 当前/前一个 token 的 end_pos)、get_start_pos/
    get_end_pos(代理 TreeInfo)、set_error_end_pos;
  * 错误恢复:ErrorRecoveryAction(trait)/BasicErrorRecoveryAction(BLOCK_STMT/
    CATCH_CLAUSE)、RECOVERY_THRESHOLD=50、skip(四 stop 条件)、syntax_error
    (JCErroneous -> report_syntax_error + set_error_end_pos + toP)、
    report_syntax_error 去重;error_key 用生成的 Errors 访问器(87 处引用);
  * mode/lastmode 解析上下文常量、type annotations pushback 状态严格复刻。
- EndPosTable 实现类(javac_parser.rs,1:1):AbstractEndPosTable(parser 引用/
  error_end_pos/set_error_end_pos/set_parser/to/toP)、SimpleEndPosTable
  (IntHashTable<TreeId,i32>,storeEnd 取较大者;getEndPos/replaceTree)、
  EmptyEndPosTable(全空);keep_end_positions 由调用方(genEndPos)决定。
- 与 JCTree 的对接(ADR 0005):F(TreeMaker)构造节点;storeEnd 记录结束位置;
  docComments 挂到 JCCompilationUnit;JavacParser 输出 JCCompilationUnit(TreeId)。

### 5. Doc 注释

- token 注释收集:JavaTokenizer 在遇到注释时调用 processComment(默认返回 None)
  并 append 到当前 token 的 comments;JavadocTokenizer 返回 JavadocComment
  (提取正文/offsetMap/@deprecated);Token.comment(JAVADOC) 取最后一个 javadoc
  (上游 classic 语义)。
- DocCommentTable(tree 层接口,ADR 0005)+ LazyDocCommentTable(parser 包实现):
  Entry { comment: Rc<dyn Comment>, tree: RefCell<Option<Rc<DCDocComment>>> }(
  上游 Entry.tree 可变、延迟解析;RefCell 为诊断层内部可变性白名单登记);
  getCommentTree -> DocCommentParser::parse;diagSource 来自 log.current_source();
  putComment/hasComment/getComment/getCommentText 1:1。
- DocCommentParser(1486 行):1:1 移植(DCTree 构造、位置、DCErroneous 错误、
  块/内联标签、HTML/实体、引用);ReferenceParser(216 行):parse(sig) ->
  Reference{module/type/member/params},内部用 ParserFactory.new_parser 解析片段;
  依赖 javac-source 的 doctree 接口层(已就绪)与 tree/DCTree(ADR 0005 S8)。

### 6. 测试策略

1. 词法/token oracle:开发期 Java 程序(--add-exports jdk.compiler/
   com.sun.tools.javac.parser,com.sun.tools.javac.util 等)对语料文件构造
   ScannerFactory/Scanner,输出 token 流(kind/pos/endPos/name/stringVal/radix/
   comments 摘要),生成对照数据入库(运行期零 JVM);覆盖 TokenKind 114 全部值 +
   语料抽样(表达式/泛型/注解/文本块/Unicode 转义/数字格式/注释/错误 token);
   单测覆盖 Unicode escape 边界(多 u/非法/eligible backslash/代理对)、数字格式
   边界、运算符最长匹配、注释收集、split、lookahead。
2. parse+Pretty 对照(ADR 0005 S7 核心验收):task-11 的 compile-only 语料全部
   parse + Pretty,与 javac oracle 逐字对照(javac-tests normalize/compare);
   语法错误用例的 Erroneous 结构/位置对照;-XDshould-stop.at=PARSE 等价性
   (task-8 已证语义:有错停在解析阶段)。
3. 结构单测:parseCompilationUnit 的 package/import/module/类型声明/成员/语句/
   表达式/类型/pattern/lambda/member reference/注解各构造(小样例 + 树结构断言);
   EndPosTable(Simple/Empty/to/toP/setErrorEndPos/keepEndPositions);
   doc comment 时序(keepDocComments 开/关、lazy 解析、位置映射);错误恢复
   (skip 停止条件、错误位置去重、RECOVERY_THRESHOLD)。
4. Doc 注释:JavadocTokenizer token 注释(text/sourcePos/@deprecated)+
   DocCommentParser/DocPretty 与开发期 Java oracle 对照(同 Pretty 方式)。
5. 覆盖率/稳定性:语料 parse 无 panic/无限循环;节点覆盖统计(ADR 0005 要求
   Kind 100%);后续 P4 差分时诊断文本逐字对齐(诊断 key/位置 P1 先校验)。

### 7. 依赖顺序(P1 内,与 ADR 0005 S1-S9 对齐)

前置:
- tree 层 S1-S5(JCTree/TreeMaker/TreeInfo/Scanner/Copier/Pretty);
- util:LayCharacters/IntHashTable/Names/Position/List/ListBuffer/Convert/Assert
  (已有);
- code 最小(1:1 完整语义,随 P1 前移):Source(版本/Feature)、Preview
  (disabledError/warnPreview/预览检查)、Lint/LintCategory(lexer/parser 的
  lexWarning/lint 检查)、Flags(已有)、Kinds(如 parser 需要);
- 诊断层最小(ADR 0004 D3-D6):JCDiagnostic/Log/DiagnosticSource 等,parser 的
  lexError/lexWarning/reportSyntaxError 依赖;需要时按 D 阶段完整移植,不写替代;
- javac-source doctree 接口层(已就绪);javac-tools JavaFileObject(内容 ->
  JavaString);生成资源表(compiler.misc.token.* 已有)。

顺序(每步 cargo test 绿、clippy 零诊断):
- S6a:Tokens(TokenKind 114 + Token/Comment/Tokens 组件);
- S6b:UnicodeReader + JavaTokenizer(词法,含 TextBlockSupport 调用)+
  JavadocTokenizer;
- S6c:Scanner/Lexer/ScannerFactory/Parser/ParserFactory(词法流水线);
- S7a:JavacParser 基础(parseCompilationUnit/import/package/类型声明/成员/
  语句/表达式 term/term3/类型/lambda)+ EndPosTable 实现类 + 错误恢复;
- S7b:JavacParser 完整语法(record/sealed/pattern/注解与类型注解 pushback/
  switch/module 声明等)与错误恢复细节;
- S8:DocCommentTable/LazyDocCommentTable/DocCommentParser/ReferenceParser
  (与 tree 层 S8 Doc 树族协作);
- S9:验证(token 流对照 + 语料 parse+Pretty 100% 对照 + 错误用例 + 覆盖率)。

### 8. 差异/风险登记

- TokenKind 114 值 -> Rust enum(保留 Java 原名 + #[allow(non_camel_case_types)],
  114 全量断言);上游 name/tag 字段用方法表达;
- Token 类层次 -> enum TokenValue(上游 String -> JavaString;comments 用
  Rc<dyn Comment>);
- UnicodeReader 输入 JavaString/UTF-16(与 javac-strings 一致;不是 UTF-8 字符串);
- JavacParser 的 mode/lastmode、type annotation pushback、错误恢复
  (RECOVERY_THRESHOLD/skip/去重)必须严格复刻,是行为等价的高风险区;
- DocCommentTable 延迟缓存需要内部可变性(RefCell,白名单登记);
- DocCommentParser/ReferenceParser 依赖 DCTree/DocTreeMaker(ADR 0005 S8);
- Source/Preview/Lint 随 P1 前移(1:1 完整最小集,不写简化替代);
- JavacParser 5039 行是最大单文件,按 S7a/S7b 两步移植并逐步用 Pretty 对照验证;
- TextBlockSupport 的 lint 行为(INCONSISTENT/TRAILING)随 JavaTokenizer 文本块
  扫描接入,警告输出在 P4 差分对齐。

## 审核修订记录(Lead 审核通过时提出,已接受并落实到实施约束)

- **① JavacParser S7a/S7b:批准。**
  - 每步语义 1:1(未覆盖语法是里程碑中间态,S7b 为最终验收);
  - 错误恢复(RECOVERY_THRESHOLD/skip 条件/去重)照抄,不得为"先跑通"弱化。
- **② Source/Preview/Lint 随 P1 前移:批准。**
  - "最小"指范围不是简化;依赖未就绪时按 ADR 0003/0004 规则(暂缺方法登记、
    同一文件后续补全);
  - 精确前置清单与当前状态见下一节。
- **③ TokenKind:批准按 PORTING_RULES §3 保留 Java 原名 +
  #[allow(non_camel_case_types)](不是 PascalCase 改名);实测 114 值写死断言
  (早期预估 118 有误,已修正);EOF(流终止符,不出现在 token 流)与 CUSTOM
  (自定义 token,正常扫描不产生)不可达,oracle 数据与测试按 114/112 记录;
  kind_name/kind_tag 映射;TokenValue 方案批准(与 Java 类层次等价)。**
- **④ LazyDocCommentTable RefCell:批准并已登记 §9.3 白名单;借用窗口短/无重入。**
- **⑤ Doc 注释顺序依赖 tree S8:接受。**
- 附加:词法 oracle 必须覆盖 TokenKind 114 全量 + 语料抽样 + 错误 token;
  JavacParser 87 处 Errors.* 用生成访问器(调用点校验器已覆盖)。

### 前置清单与当前状态(修订 ② 落实)

| 前置项 | 当前状态 | 来源/说明 |
|---|---|---|
| tree 层 S1-S5(JCTree/TreeMaker/TreeInfo/Scanner/Copier/Pretty) | 未开始 | ADR 0005;ID/arena 契约由 Lead 先落地,S1 依赖它 |
| util(List/ListBuffer/Position/Names/IntHashTable/LayoutCharacters/Convert/Assert/Context/JavacMessages/MessageFormat) | 已具备 | javac-core util/ 已移植 |
| code Flags | 已具备 | code/flags.rs |
| code TypeTag/BoundKind/TargetType | 已具备 | code/*.rs |
| code Kinds | 未移植 | parser 若需要随 P1 前移(1:1 完整) |
| code Source/Preview/Lint | 未移植 | parser 需要,随 P1 前移(1:1 完整最小集,范围最小不简化) |
| 诊断层最小(JCDiagnostic/Log/DiagnosticSource/Options) | 未开始 | ADR 0004 D3-D6;parser 的 lexError/lexWarning/reportSyntaxError 依赖 |
| javax.tools 接口层(JavaFileObject/JavaFileManager) | 已具备 | javac-tools |
| com.sun.source 接口层(含 doctree) | 已具备 | javac-source |
| 生成资源表(compiler.misc.token.* 等) | 已具备 | task-19/23 resources/ |
| tree/token oracle 数据 | 本任务(task-26)产出 | tools/gen-tree-oracle、tools/gen-token-oracle + javac-tests/data |

## 结果

- parser 层 13 个文件 -> Rust 模块映射与 1:1 规则明确;
- Token 模型(114 TokenKind/Token/Comment/Tokens)、词法层(UnicodeReader/
  JavaTokenizer/Scanner/工厂)、parser 层(JavacParser/EndPosTable 实现类)、
  doc comment(JavadocTokenizer/LazyDocCommentTable/DocCommentParser/
  ReferenceParser)设计明确;
- 与 ADR 0005 的 arena+ID 树表示对接契约明确;
- 测试以 token 流 oracle + parse+Pretty 逐字对照为核心,附结构化单测与覆盖率;
- P1 内 S6-S9 顺序、依赖清单(含 Source/Preview/Lint 与诊断层最小前置)与风险
  登记明确。
