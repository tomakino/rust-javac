# 目录说明

```
crates/                 编译器及支撑 crate
  javac-core/           解析、enter、attr、flow、lower、代码生成、诊断
  rust-javac/           CLI 二进制与 CLI 测试
  javac-tools/          javax.tools 风格的文件管理器 API
  javac-source/         com.sun.source 树 API
  javac-unicode/        Unicode 表
  javac-floating/       浮点/BigDecimal 辅助
  javac-jimage/         jimage 读取
  javac-jmod/           jmod 读取
  javac-zip/            zip 读取
  javac-ctsym/          ct.sym 读取
  javac-classpath/      classpath 处理
  javac-charset/        字符集支持
  javac-fs/             文件系统辅助
  javac-tests/          语料/diff/oracle 工具
xtask/                  cargo xtask 生成器
tools/                  Java oracle 生成器
docs/                   文档
```

`crates/javac-core/resources/l10n/` 保存官方 `.properties` 语言包，由
`include_str!` 嵌入二进制，运行时惰性解析。
