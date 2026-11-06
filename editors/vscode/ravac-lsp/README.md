# Ravac LSP for VS Code

把 ravac-lsp 接到 VS Code：补全、诊断、悬停、跳转定义、引用、文档符号、内联提示、签名帮助。

## 安装

在扩展目录执行：

    npm install
    npx @vscode/vsce package --allow-missing-repository
    code --install-extension ravac-lsp-0.1.0.vsix --force

或直接把本目录（含 node_modules）复制到：

    %USERPROFILE%\.vscode\extensions\local.ravac-lsp-0.1.0

## 配置

工作区设置：

    "ravacLsp.serverPath": "${workspaceFolder}/target/debug/ravac-lsp.exe",
    "ravacLsp.classPath": "${workspaceFolder}/target/tmp/perf/javac",
    "ravacLsp.sourceLevel": "17"

如果使用 Maven 项目，设置 ravacLsp.projectPath 为项目根目录，服务器会追加依赖。

## 与 redhat.java 共存

本扩展注册 Java 语言的补全。redhat.java 也注册补全，会混在一起。
本仓库的 .vscode/settings.json 已关闭：

    java.completion.enabled
    java.signatureHelp.enabled
    java.inlayHints.*

保留 redhat.java 的语法高亮、项目模型、调试和构建。

## 服务器权威

服务器已经用 IDEA 的 MinusculeMatcher 过滤并排序。客户端：

- 用 sortText 固定服务器返回顺序，避免 VS Code 按 label 重排；
- 用当前前缀填充 filterText，避免 VS Code 二次模糊过滤掉中间匹配；
- isIncomplete 为 true，每次按键重新请求服务器。

## 命令

- Ravac: 重启语言服务器
- Ravac: 显示输出
- Ravac: 探测当前补全
