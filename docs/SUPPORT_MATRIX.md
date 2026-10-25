# 支持矩阵(SUPPORT_MATRIX)

本文件登记与 javac 17 的已知差异:裁剪的命令行选项、被裁剪的行为、不可对齐的调试输出。
除本表所列,rust-javac 以 javac 17.0.19(openjdk/jdk17u @ 982d53c)为唯一行为基准。

## 命令行选项

保留(与 javac 对齐):源文件、@argfile、-d、-classpath/-cp、-sourcepath、--module-path/-p、
--upgrade-module-path、--system、--release、-source、-target、-encoding、-g、-g:none、
-parameters、-verbose、-nowarn、-deprecation、-Xlint、-Xlint:all、-Werror、-Xmaxerrs、
-Xmaxwarns、-Xdiags、-XdrawDiagnostics、-XDshould-stop.at、-version、-fullversion、--help、
--help-extra、-X、--enable-preview、--module/-m、--add-modules、--add-exports、--add-reads、
--patch-module、--module-source-path、-implicit。

裁剪(传入时明确报不支持并退出,不伪造行为):
- AAPT 相关:全部 -processor*、-proc:* 选项(注解处理器不实现)。
- -h(JNI 头文件生成)、-s(生成源输出,APT 用)、-J*(JVM 参数)。
- javadoc 专有/废弃项:-bootclasspath、-profile、-doclet 等(以 Option.java 逐项核对)。
- 其余 -X 内部调试项(除差分测试需要的 -XDshould-stop.at 等)。

## 行为裁剪

- 注解处理器(javax.annotation.processing):不实现。默认不扫描/运行处理器。
- com.sun.tools.javac.launcher(单文件源码启动器 java Foo.java):不移植。
- com.sun.tools.javac.processing 包:不移植。

## 不可对齐的调试输出

- GraphUtils.DottableNode/DotVisitor/toDot:DOT 调试输出依赖 Object identity hash 与
  Hashtable/Properties 迭代顺序,无法与 javac 逐字对齐,且仅用于 -X 调试。已裁剪;
  如需该调试功能另行评估。

## 平台适配层限制

- javac-jimage:仅支持 jimage 1.0 与小端字节序(基线 JDK lib/modules 即本机生成);不支持
  jlink --compress 压缩资源(基线 image 无压缩资源);packages() 树按 P2 需要补齐。

- javac-jmod:仅 jmod 格式 1.0(照抄 JmodFile.checkMagic:接受 0.x/1.0,拒绝 1.1+/2.x);
  纯容器层不解析 class;模块名取自文件名。读取依赖 deflate(jmod classes 条目为 deflate)。
- javac-zip(在实现):标准 zip(central directory/stored/deflate/UTF-8 flag);zip64 明确报错
  直到需要;不使用 zip crate(避免平台构建前置)。

- javac-zip(自研,替代 zip crate):仅标准 zip(central directory/stored/deflate/UTF-8 flag,
  无 flag 按 IBM437);zip64 与加密明确报错;CRC 只暴露不校验;读取错误可折叠为 None(get_*)或
  read_entry 取错误。

## 语料与基线(javac-tests)

- 语料来源:reference/jdk17u 的 test/langtools/tools/javac(sparse checkout;不入项目 git)。
- 分类:compile-only 1955(用于 oracle 基线)、compile+run 394、modules 1053、unavailable 175、
  needs-library/APT 12、非测试 1584;排除原因逐用例记录。
- 基线内容只含 oracle 行为(exit/stdout/stderr/class 清单,规范化),不含 jtreg .out 比对;
  差分留给 P9。非 UTF-8 源文件当前记为 Unsupported。
