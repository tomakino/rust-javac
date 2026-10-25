# 依赖登记(DEPENDENCIES)

规则:仅 GPLv2 兼容许可(MIT/BSD/ISC/Zlib 等;避免 Apache-2.0 单许可),纯 Rust,无 JVM 依赖,
Android 可构建;每个依赖登记用途与替代的 JDK 能力。

| crate | 版本 | 许可 | 用途 | 替代的 JDK 能力 | 状态 |
|---|---|---|---|---|---|
| (暂无) | | | | | |

已否决:
- zip crate 9.1.0:引入 windows-sys/getrandom 的 raw-dylib,使 Windows GNU 首次构建依赖
  dlltool,不适合 CI/Android/开源可复现构建。已改为自研 javac-zip,依赖已从 workspace 移除。

已准入:
| crate | 版本 | 许可 | 用途 | 替代 JDK 能力 |
|---|---|---|---|---|
| miniz_oxide | 0.8.9 | MIT / Apache-2.0(按 MIT) | crates/javac-zip 的 DEFLATE 解压 | java.util.zip 解压部分 |
| adler2 | 2.0.1 | 0BSD / MIT / Apache-2.0 | miniz_oxide 传递依赖 | - |

依赖树验证:workspace 中无 zip/windows-sys/getrandom/windows-link;在无 dlltool 的 PATH 下
clean 构建 javac-zip/javac-jmod 通过。
