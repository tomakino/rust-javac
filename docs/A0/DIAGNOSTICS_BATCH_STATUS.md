# 诊断接线批次状态(D0-D9,防压缩保护)

维护:util-porter;提交:Lead。最后更新:本轮(D1 返工完成,工作树全绿)。

## D0 审计 v3(完成,Lead 已提交)
- 产物:`docs/AUDIT_DIAGNOSTICS_TABLE.md` 第三版(935 工厂全量五类状态 + 清单)。
- 统计:已接线 639;待接线 67;未移植 40;范围外 24(processing,PLAN §3.1);无上游触发 165;合计 935。
- D 批次映射:D1 Attr.multicatch;D2 Attr.try-with-resources;D3 Flow guard/inner.cls;D4 Resolve 8;D5 ClassReader 5;D6 Locations 8;D7 其余 42。

## D1 多 catch + 返工(完成,工作树全绿,待 Lead 审核提交)
### 功能实现(1:1)
- `comp/attr.rs`:visit_type_union(Attr.visitTypeUnion 全部控制流)+ Tag::TYPEUNION dispatch。
- `code/type_.rs`:Type::new_union_class_type(Type.UnionClassType 构造器 1:1)。
- `comp/check.rs`:e2e multicatch_disjoint_types_compile、multicatch_related_types_report_disjoint_error;新增单元断言 supertype_completion_registers_class_in_real_symtab(supertype 触发 completion 后真实 Symtab 可见 completed Exception/IOException)。
- `jvm/codegen.rs`:TryFinalizer.has_finalizer 按上游 tree.finalizer!=null 修复 + after_body(BODY_ONLY_FINALIZE)。
### B 变体返工(Lead 审核:clone Symtab 适配无效,已按要求重做)
- 根因:上游 Symbol.complete 不需要 Symtab 可变引用;本地把完成建模为 (&mut Symtab) 才是偏差。
- `code/symtab.rs`:可变表 SymtabMaps/class_fields 改为 Rc<RefCell<...>> 内部可变(不 clone);Symtab/SymtabMaps 删除 Clone derive;写方法 enter_class_flat/enter_class/lookup_package*/enter_package/enter_module/define_class/get_class_field/remove_class 等改 &self 短借;读方法 get_class/get_package/get_module 等短借后释放。
- 完成链签名还原上游语义:`Completer::complete(ctx,syms:&Symtab,sym)`;ClassFinder(14)/ClassReader(55)/ModuleFinder(7)/Symbol(8) 等 &mut Symtab→&Symtab;CompilerContext::complete_symbol/complete_class_symbol 接受 &Symtab;全项目类型级 `&mut Symtab` 归零(799→0);Completer 各实现(Flag/ClassFinder/Deferred/TypeEnter/Main/Source/Unnamed/UsesProvides)同步。
- `code/types.rs`:Types.supertype 在读取 superclass 前调用 ctx.complete_class_symbol(syms, tsym)(删除 clone 适配);completion 写入真实 Symtab maps。
- 变更面:30 文件,2434 insertions/2184 deletions。
### 验证(本轮实际执行)
- cargo check -p javac-core → 0 errors
- cargo test --workspace --all-targets → 1114 passed / 0 failed / 无 already borrowed panic
- e2e:multicatch 2 ok;supertype_completion 1 ok
- clippy --workspace --all-targets → clippy_errors=0,warnings=207(低于 D1 前 214 基线;本批 30 文件 60 条,均为既有风格类/测试目标差异,已登记)
- unimplemented!/unreachable! 全项目 0
- JDK17 oracle 重跑:合法 multi-catch exit=0;非法报 compiler.err.multicatch.types.must.be.disjoint(参数顺序一致)。
### 过程事故申报
中途被外部停止后恢复工作树时,一次基于 clippy 建议的批量自动应用因字符串/字节偏移错误与 --all-targets 的 lib/test 目标差异,损坏了 type_/attr/check/symtab/symbol/type_annotations 若干文件与 module_finder 的 test 依赖 import;已通过 git HEAD 原文 + 确定性重放(含 D1 代码块重建)恢复,未使用 git checkout/reset,未回退他人改动;误建两个空文件(code/modules.rs、code/resolve.rs)已删除。最终全绿。
- 下一步:D2(try-with-resources)。
