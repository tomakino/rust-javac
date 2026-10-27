# 诊断接线批次状态(D0-D9,防压缩保护)

维护:util-porter;提交:Lead。

## D0 审计 v3(完成)
- 产物:`docs/AUDIT_DIAGNOSTICS_TABLE.md` 第三版(1148 行,935 工厂全量 + 五类状态 + 清单)。
- 口径:精确模块匹配(显式 `<module>::<factory>(` 或导入该模块时裸调用;修正 v2 同名跨模块误报 11 条);上游诊断点 file:line(重扫 5902 Java 文件);五类互斥。
- 统计:已接线 639;功能已具备待接线 67;功能未移植(批次)40(Main/Options 32、API 5、Model 1、Util 2);范围外 24(processing,PLAN §3.1);无上游触发 165;合计 935。
- 与 v2 差异:已接线 647→639(更正 11 条同名跨模块误报 + 新增 3 条 A0/A1/A0b 接线 instanceof×2、varargs.trustme.accessor);未移植 66→40+范围外 24;无上游触发 155→165。
- 待接线批次映射:D1 Attr.multicatch;D2 Attr.try-with-resources;D3 Flow guard/inner.cls(2);D4 Resolve 8(verbose 5+fatal 3);D5 ClassReader 5;D6 Locations 8;D7 其余 42。

## D1 多 catch(完成)
- 实现(1:1):
  - `comp/attr.rs`:新增 `visit_type_union`(上游 Attr.visitTypeUnion 全部控制流:逐 alternative attrib + checkClassType + checkType Throwable、pairwise subtyping 检查、multicatch_types/all_multicatchTypes 惰性列表、lub、check(NO_TREE_UPDATE)、UnionClassType 包装、tree.type/result);dispatch 注册 Tag::TYPEUNION。
  - `code/type_.rs`:新增 `Type::new_union_class_type`(上游 Type.UnionClassType 构造器:outer/typarams/allparams/supertype/interfaces 复制,`all_interfaces_field = interfaces_field` 上游语义;CLASS tag,tsym 继承)。
  - 诊断接线:`compiler.err.multicatch.types.must.be.disjoint`(`errors::multicatch_types_must_be_disjoint`,TypeArg 占位按 P4 现有约定)。
  - e2e(`comp/check.rs`):`multicatch_disjoint_types_compile`(合法 disjoint 编译无 error)、`multicatch_related_types_report_disjoint_error`(IOException|FileNotFoundException 断言诊断键)。
- 依赖修复(真实偏离,非特判):
  - `jvm/codegen.rs`:TryFinalizer.has_finalizer 原硬编码 true,上游为 `tree.finalizer != null`;补 `has_finalizer`/`body_only_finalize` 字段与 after_body(BODY_ONLY_FINALIZE 时移除 finalize)。修复 try 无 finally 时 genTry 走 catch-all 并 finalizer_pos panic。
  - `code/types.rs`+`code/symtab.rs`:Types.supertype 在上游经 ClassSymbol.getSuperclass 隐式 complete 符号;本地原缺 completion(登记缺口被 D1 暴露:Exception 符号未完成→supertype noType→IOException !<: Throwable→prob.found.req)。按上游语义在读取 superclass 前完成符号;因 Types 方法签名是 &Symtab,完成时 clone Symtab(ClassFinder 只写 arena 值、只读 Symtab;Port adapter,待 Lead 审核登记 §9);Symtab/SymtabMaps 增加 Clone。
- 验证(本轮实际执行):cargo check 0 errors;cargo test --workspace --all-targets 1113 passed/0 failed;clippy --workspace --all-targets errors=0;unimplemented!/unreachable!=0(全项目 0);D1 e2e 2/2 通过;JDK17 oracle:合法 exit=0,非法报 `compiler.err.multicatch.types.must.be.disjoint: java.io.FileNotFoundException, java.io.IOException`。
- 偏差登记(§9):Symtab clone adapter(Types.supertype completion);诊断参数 TypeArg::placeholder(type_to_string)属 ADR 0002 P4 现有占位约定;visitTypeUnion 的 attrib_type_use 对应上游 attribType(既有本地等价入口)。
- 未实现:无。
- 下一步:D2 try-with-resources(Attr try.resource.may.not.be.assigned + 相关 TWR 路径验证)。
