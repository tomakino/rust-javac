# DEBUG: record/field annotation stack overflow (record R(@SafeVarargs int x)) — 精确定位记录

状态:临时调试文档(Lead 要求),证据用于压缩后继续;临时插桩已清理,工作树不含本文件之外的调试代码。

## 复现

```powershell
cd C:/Users/Lin/RustroverProjects/rust-javac
$env:JAVAC_DEBUG_STACK="1"
C:/Users/Lin/.cargo/bin/cargo.exe test -p javac-core --lib comp::check::diagnostics_tests::tmp_type_annotations_diag -- --nocapture 2>log
```

最小源码(临时测试):`class R { @Deprecated int x; }`;record 变体:`record R(@SafeVarargs int x) {}`。

## 插桩点(临时,已清理)

- code/type_annotations.rs:`TypeAnnotationPositions.scan/visit_*` 链上:
  - `separate_annotations_kinds` 入口(SEP_ENTER:sym/ty/anns 数)、每条注解目标(SEP_TARGET:a/target)、type_annos 空判定(SEP_TYPE_ANNOS);
  - `type_with_annotations`(TWA_ENTER:ty/typetree/anns)、`type_with_annotations_upto`(TWAU_ENTER:depth/ty/stop;depth>100 打 backtrace+panic)、`rewrite_array_type`(RAT_ENTER)、`to_type_compound`(TTC_ENTER);
  - `type_with_annotations` 两处 while 的迭代计数(enclosing walk/TOP walk),>100 打 backtrace+panic;
- comp/attr.rs:`check_for_declaration_annotations` 入口(CFDA_ENTER);
- 全部由环境变量 JAVAC_DEBUG_STACK 控制,不设置时零输出。

## 原始输出(尾部,实跑)

```
SEP_ENTER sym=808 ty=Some(15) anns=1
SEP_TARGET a=1 target=DECLARATION
SEP_TYPE_ANNOS empty=true type_len=0 only_len=0
thread '<unknown>' ... has overflowed its stack   (STATUS_STACK_OVERFLOW)
```

结论:
1. 字段 sym=808 的唯一注解(@Deprecated)目标判定为 **DECLARATION**(不是 BOTH/TYPE),type_annos 为空 → `separate_annotations_kinds` 在 type_annos 空分支**正常早退**,不会进入 `type_with_annotations` 链;插桩 TWA/TWAU/RAT/TTC/CFDA 全部未打印。
2. 因此栈溢出不在 deproxy/AnnotationTypeMetadata/Annotate 队列/Check.validate_annotations,也不在 type_with_annotations 链;它在 `separate_annotations_kinds` 返回后、`check_for_declaration_annotations` 进入之前的**类型注解扫描器(TypeAnnotationPositions 树扫描)后续路径**(type_annotations.rs:624-840 的 visit_* 调用点之后;字段路径为 :797 visit_var_def)。
3. 主动更正:此前 deproxy 循环推断不成立(deproxy 完整返回);type_with_annotations 假设亦被本证据排除(type_annos 空早退)。
4. 下一步:在 `TypeAnnotationPositions.scan` 与字段路径 visit_var_def(:765) 出口、tree_scanner visitor 委托(`tree_scanner_visitor_delegate!`,:1130)以及调用方(Attr 的 TypeAnnotationsValidator.visit/validateAnnotatedType)加进出 guard,取 SEP 返回后第一个未返回函数;再对照上游 TypeAnnotations.java:300-560 / Attr.java:5795-5806 修复。

## 备注

- 上一条已确认:ATT_END(annotation_target_type 完成、8 个 target 全部终判)之后即爆栈;本轮进一步证明其后只有早退路径。
- 工作树:插桩与临时测试在提交前必须全部清理(已按此执行)。
