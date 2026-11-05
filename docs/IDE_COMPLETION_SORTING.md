# 补全排序设计（对齐 IDEA 的候选顺序）

> 状态：设计稿（待确认，未实现）
> 关联：docs/IDE_COMPLETION.md（候选规格）、docs/IDE_COMPLETION_INSERT.md（插入规格）

---

## 1. 现状与问题

截图对比（System. 的补全）：

**Sublime（我们）**：Logger、LoggerFinder、arraycopy、clearProperty、console、currentTimeMillis、err、exit、gc、getLogger …

**IDEA**：out、in、arraycopy、clearProperty、console、currentTimeMillis、exit、gc、getenv …

差异：

1. **字段没有优先**：我们按 label 字母序（Logger < arraycopy），字段 err 落到中间。
2. **类混在字段/方法里**：Logger/LoggerFinder（嵌套类）排最前，IDEA 不显示它们。
3. **重载位置不稳定**：同一方法的多个重载按 label 相同，顺序由扫描顺序决定。

根因：completion_impl 的 receiver 分支只做 sort_by_key(label)（字母序），非 receiver 分支有 kind 权重但两处规则不统一。

---

## 2. 目标

1. **字段优先**：同一 receiver 下，字段（out/in/err）排在方法前。
2. **方法次之**：按名字的自然序。
3. **类/接口再次**：仅当它们可被引用（嵌套类已按既定规则过滤，非当前文件不出现）。
4. **关键字最后**（非 receiver 上下文）。
5. **重载相邻且稳定**：同 label 的不同签名按参数顺序排列，不随扫描顺序抖动。
6. **精确匹配优先**：label == prefix 的项排最前。
7. 服务端顺序即最终顺序（Sublime LSP 设置了 INHIBIT_REORDER，保留服务端顺序）。

---

## 3. 排序键（统一）

```
sort_key = (
    exact_match,     // 0: label == prefix；1: 前缀匹配
    kind_rank,       // 见下表
    name_key,        // 自然序（大小写不敏感，再按原串）
    signature,       // label_detail，稳定重载顺序
)
```

### 3.1 kind_rank

| 候选类型 | rank | 依据 |
|---|---|---|
| LocalVariable / Parameter | 0 | 局部作用域最相关 |
| Field | 1 | IDEA 字段优先（out/in） |
| Method | 2 | |
| Class / Interface / Enum / Record | 3 | |
| Keyword | 4 | |
| 其他 | 5 | |

> System. 是静态 receiver，只有静态字段/方法参与，顺序即 Field → Method。

### 3.2 name_key（自然序）

- 大小写不敏感比较；相同则按原串比较（Logger vs logger）。
- 不引入 locale 依赖（ASCII 小写比较 + 原串兜底）。

### 3.3 exact_match

- prefix 为空：全部为 1（不参与区分）。
- label == prefix：0，否则 1。
- 目的：输入 gc 时 gc() 不被 get... 挤下去。

---

## 4. 特殊规则

1. **重载**：同 label、同 kind 的多个方法按 label_detail（参数签名）排序；参数少者在前（print() 在 print(Object) 前）。
   - 实现：signature_key = (argument_count, signature_text)。
2. **静态/实例**：static_only 的过滤已在候选阶段完成，排序不区分。
3. **去重**：(label, kind, label_detail) 相同才去重（已有）。
4. **截断**：排序后截断 100 条（保持现状）。
5. **symbols() 分支**（非 receiver）：同样使用统一排序键，删除现有重复排序逻辑。

---

## 5. 客户端约束（已核对 LSP 2.16）

- format_completion 不使用 sortText，trigger 由 label/labelDetails 拼成。
- items 非空时设置 INHIBIT_REORDER，Sublime 保留服务端顺序。
- 结论：**服务端排序即最终显示顺序**；不需要额外的 sortText。

---

## 6. 实现位置

```
crates/javac-ide-core/src/analysis/features/completion/mod.rs
  fn sort_items(items: &mut Vec<CompletionItem>, prefix: &str)
```

- receiver 分支与 symbols() 分支统一调用。
- 删除两处内联排序代码。
- 排序为稳定排序（Rust 的 sort_by 是稳定排序）。

---

## 7. 测试计划

| 用例 | 断言 |
|---|---|
| S23 | System. 的 out/in/err 在 arraycopy 之前 |
| S24 | System. 的方法按自然序：arraycopy < clearProperty < currentTimeMillis |
| S25 | 类候选（若出现）排在同级方法之后 |
| S26 | 重载相邻，且参数少者在前 |
| S27 | 输入 gc 时 gc() 排第一（精确匹配优先） |
| 回归 | 现有 59 项规格（只断言存在性，不受排序影响） |

---

## 8. 边界

- **locale**：只做 ASCII 大小写折叠，避免系统语言影响排序。
- **稳定性**：同键项保持输入顺序（声明顺序），保证结果可复现。
- **性能**：排序 O(n log n)，n ≤ 100（截断前最多几百），无额外开销。

---

## 9. 已确认决策（2026-11-06）

| 决策 | 结果 |
|---|---|
| 字段优先 | **是**（字段 rank 1 < 方法 rank 2） |
| 大小写排序 | 用户跳过 → 采用**大小写不敏感**（默认推荐） |
| 重载顺序 | **参数少者在前**（参数数 → 签名文本） |

---

## 10. IDEA 源码依据（2026-11-06 查证，链接见文末）

### 10.1 排序（platform/analysis-impl）

- **BaseCompletionLookupArranger.sortByPresentation**：先按 `itemMatcher.isStartMatch(element)` 把项分成 `startMatches` / `middleMatches`，两组各自按 presentation 排序，**startMatches 整组在前**。
- **PreferStartMatching**（`LookupElementWeigher`）：`weigh(element) = !CompletionService.isStartMatch(element)`——开始匹配权重为 false，排前。
- **CompletionSorter / weighBefore / weigh**：可插拔权重链；最终 `CompletionFinalSorter` 统一（供 ML 排序插件）。
- 结论：IDEA 排序 = **start match 分组** → **权重链** → **presentation 排序**，不是单一比较器。

### 10.2 匹配（platform/util/text-matching）

- **NameUtil.buildMatcher / MinusculeMatcher**：驼峰匹配（`gSM` → `getSecurityManager`），大小写不敏感，`MatchingMode.FIRST/LAST`，`matchingDegree` 打分。
- 结论：服务端过滤必须是**驼峰子序列**，不能只 `starts_with`，否则 `gSM` 类输入没有任何候选。

---

## 11. 修正后的排序/匹配规则（对齐 IDEA）

### 11.1 排序键（更新 §3）

```
sort_key = (
    !start_match,    // 0: label 以 prefix 开头；1: 中段匹配
    !exact,          // 0: label == prefix
    kind_rank,       // §3.1
    name_key,        // §3.2
    signature,       // §3.3
)
```

- start_match 放第一位，与 IDEA 的 `sortByPresentation` 分组一致。
- exact 仍保留（`gc` 输入时 `gc()` 排第一）。

### 11.2 过滤（更新）

- prefix 为空 → 全部候选。
- 否则 → `camel_match(prefix, label)`（子序列 + 驼峰边界）。
- 匹配质量用于排序，不做二次过滤。

### 11.3 camel_match 规格（待实现）

1. 大小写不敏感；prefix 的字符按顺序出现在 label 中（子序列）。
2. 驼峰边界（大写字母、`_`、`$`、`.` 后的字符）命中加分（对应 IDEA 的 matchingDegree）。
3. `*` 通配第一版不做。
4. 匹配度只影响排序（start_match 已经覆盖大部分场景），不改变候选集合。

### 11.4 新增测试

| 用例 | 断言 |
|---|---|
| S28 | 输入 `gSM`，`getSecurityManager` 出现在候选中 |
| S29 | `Sy` 的候选中 `System` 存在且 start match 组在前 |
| S30 | 输入 `ystem`（中段匹配）时 `System` 仍出现，但排在 start match 项之后 |

---

## 12. 源码链接

- CompletionSorter: https://github.com/JetBrains/intellij-community/blob/master/platform/lang-api/src/com/intellij/codeInsight/completion/CompletionSorter.java
- BaseCompletionLookupArranger: https://github.com/JetBrains/intellij-community/blob/master/platform/analysis-impl/src/com/intellij/codeInsight/completion/BaseCompletionLookupArranger.java
- PreferStartMatching: https://github.com/JetBrains/intellij-community/blob/master/platform/analysis-impl/src/com/intellij/codeInsight/completion/impl/PreferStartMatching.java
- CompletionLookupArranger: https://github.com/JetBrains/intellij-community/blob/master/platform/analysis-impl/src/com/intellij/codeInsight/completion/CompletionLookupArranger.java
- CompletionFinalSorter: https://github.com/JetBrains/intellij-community/blob/master/platform/analysis-impl/src/com/intellij/codeInsight/completion/CompletionFinalSorter.java
- MinusculeMatcher: https://github.com/JetBrains/intellij-community/blob/master/platform/util/text-matching/src/com/intellij/psi/codeStyle/MinusculeMatcher.kt
- CamelHumpMatcher: https://github.com/JetBrains/intellij-community/blob/master/platform/analysis-impl/src/com/intellij/codeInsight/completion/impl/CamelHumpMatcher.java