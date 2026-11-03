# zh_Hant 消息翻译标准

适用范围：`resources/l10n/{compiler,javac,launcher}_zh_Hant.properties`（共 1097 条消息）。
目标语言：**通用繁体中文**（一套服务台湾、香港、澳门），不引入任一地区的特有用语。

## 1. 基准与优先级

1. **英文原文为准**：`compiler.properties` / `javac.properties` / `launcher.properties` 是语义基准。
2. **zh_CN 作参考**：简体译文用于对齐术语与句式，但不得直接字形转换后照抄。
3. 消息键（`compiler.err.xxx`）、注释行（`#`）**不翻译、不改动**。

## 2. 语言规范

### 2.1 字形

- 全文使用繁体字形（後、讓、這、類、檔、錯、誤……）；不残留简体字。
- 一简对多繁必须按语境选择：「以后→以後」「皇后→皇后」「头发→頭髮」「发展→發展」
  「干净→乾淨」「干扰→干擾」「树干→樹幹」。

### 2.2 用词（通用优先，避开地区特有词）

- 以第 2.3 节术语表为准。
- 大陆特有表达改为通用繁体：信息→資訊、软件→軟體、默认→預設、程序→程式、对象→物件、字符串→字串。
- 括号内保留英文原名时，英文不翻译：`String`、`int`、`class`、`-Xdiags`、`--release`。

### 2.3 术语表（必须统一）

| 英文 | 统一译法 |
|---|---|
| class | 類別 |
| method | 方法 |
| field | 欄位 |
| variable | 變數 |
| type | 類型 |
| type argument | 類型引數 |
| type parameter | 類型參數 |
| generic | 泛型 |
| wildcard | 萬用字元 |
| inference | 類型推斷 |
| capture | 捕獲 |
| erasure | 類型擦除 |
| annotation | 註解 |
| statement | 陳述式 |
| expression | 運算式 |
| block | 區塊 |
| loop | 迴圈 |
| exception | 例外 |
| error | 錯誤 |
| warning | 警告 |
| note | 注意 |
| overload | 多載 |
| override | 覆寫 |
| constructor | 建構子 |
| instantiate | 實例化 |
| interface | 介面 |
| package | 套件 |
| module | 模組 |
| file | 檔案 |
| directory | 目錄 |
| class file | 類別檔 |
| constant pool | 常數池 |
| descriptor | 描述子 |
| signature | 簽章 |
| compile | 編譯 |
| parse | 解析 |
| attribution | 屬性檢查 |
| flow analysis | 流程分析 |
| desugar | 解糖 |

## 3. 格式规范（必须逐字保留）

1. **占位符**：`{0}`、`{1}` … 原样保留，编号、数量、顺序不得改变；不得新增或删除。
2. **转义**：`\n`、`\t`、`\r`、`\f`、`\\` 原样保留；换行位置与原文一致。
3. **引号与括号**：英文引号 `'…'`、`"…"` 保留；中文文本用全角括号 `（）`、全角标点 `，。：；、`。
4. **空格**：与 zh_CN 风格一致（不额外加空格）；英文单词/占位符两侧的空格按英文模板。
5. **大小写**：句首大写规则不适用中文；英文专有名词保持原大小写。
6. 一条消息的键与值在同一行；不使用续行（原文件无续行）。

## 4. 语气与句式

- 简洁、客观，接近 javac 官方消息风格；不用「您」「请」等客套语。
- 省略主语（「找不到符號」而非「我們找不到符號」）。
- 冒号后接详情，分号分隔并列项，与英文结构对应。
- 不添加原文没有的解释、提示或示例。

## 5. 交付与校验

1. 按 100 条/批翻译；每批写回 `*_zh_Hant.properties`（UTF-8 明文）。
2. 每批校验：占位符集合与英文模板一致；无简体字；无地区特有用语。
3. 全量完成后：
   - `cargo test -p javac-core --lib`（本地化相关测试）；
   - `LANG=zh_TW.UTF-8` / `zh_HK` / `zh_MO` 编译样例，确认输出为繁体；
   - `LANG=zh_CN.UTF-8` 确认简体不受影响。
