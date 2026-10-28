# 官方 javac 关键调用链（小项目 JDI 采集）

Status: reference only（不参与构建；用于 A3l 审核对照）
Date: 2026-10-28
Source: target/fake-project/calls3.txt（临时产物，不入库）

## 1. 样本

`target/fake-project/src/p/` 共 8 个文件：

- `A.java`：最小类
- `Dep.java`：`@Deprecated class Dep {}`
- `StaticImp.java`：`import static java.lang.Math.max`
- `StarImp.java`：`import java.util.*`
- `Generic.java`：泛型方法/推断
- `Flow.java`：for-each + try-with-resources
- `Lambda.java`：lambda
- `Enum.java`：enum

官方 javac：`exit 0`，生成 8 个 class。

## 2. 采集方式

- `java -agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=127.0.0.1:5005 -m jdk.compiler/com.sun.tools.javac.Main ...`
- JDI（`target/fake-project/JDI/TraceCalls.java`）attach 后，为 8 个关键类各建一个 `MethodEntryRequest`（不是多个 class filter 挂在同一请求上），`SUSPEND_EVENT_THREAD`，读取 `ThreadReference.frames()`。
- 关键类：`JavaCompiler / Enter / TypeEnter / MemberEnter / Attr / Resolve / Infer / Scope$ScopeImpl`。
- 汇总：`events=24119 javacEvents=24117 edges=451`；JDI 正常 `VMDisconnect` 退出。

**覆盖范围**：这 8 个类的方法入口事件；其他类（ClassReader/ClassFinder/Modules/TreeMaker 等）只作为栈帧出现，没有单独设事件。它不是“所有类所有方法”的全量。

## 3. 关键调用链

### 3.1 编译骨架

```
JavaCompiler.compile
  Main.compile
    Main.main

Enter.main
  JavaCompiler.enterTrees
    JavaCompiler.compile

Enter.complete
  Enter.main

TypeEnter.complete
  Symbol$ClassSymbol.complete
    Enter.complete

MemberEnter.signature
  MemberEnter.visitMethodDef
    MemberEnter.memberEnter
      TypeEnter$MembersPhase.finishClass
        TypeEnter$MembersPhase.runPhase
          TypeEnter.complete
```

### 3.2 @Deprecated / 注解类型归因（A3l 直接对照点）

```
Attr.attribTree
  Attr.attribType
    Attr.attribType
      Attr.attribAnnotationTypes
        TypeEnter.markDeprecated
          TypeEnter$HierarchyPhase.runPhase
            TypeEnter.complete
```

这说明官方在 `markDeprecated` 中确实通过 `attribAnnotationTypes → attribType → attribTree` 解析 `@Deprecated`；本地 chaining 下 `Deprecated` 的 `ABSENT_TYP` 应沿这条链检查 imports/star scope。

### 3.3 Scope.leave 的 classfile 路径

```
Scope$ScopeImpl.leave
  ClassReader.readClass
    ClassReader.readClassBuffer
      ClassReader.readClassFile
        ClassFinder.fillIn
          ModuleFinder$1.complete
            Symbol.complete
```

说明 classfile 读取过程中也会 `leave` 临时 scope；本地 leave 配对/id 复用时必须兼容这条路径。

### 3.4 统计

- `Scope$ScopeImpl.enter`：3032 个栈
- `Scope$ScopeImpl.leave`：222 个
- `Attr.attribTree`：133
- `Resolve.findGlobalType`：40
- `MemberEnter.signature`：15
- `Resolve.findType`：14
- `Attr.attrib`：8
- `TypeEnter.complete`：8
- `Enter.complete`：1
- `Enter.main`：1
- `JavaCompiler.compile`：1

## 4. 用途与边界

- 作为 A3l 审核的官方路径 oracle：检查本地 `TypeEnter`/imports/`Resolve`/`Scope` 的调用顺序与栈关系。
- 不用于断言“所有方法都被采集”；全量所有类需要 JVMTI/agent 或让通配符 JDI 完整跑完。
- 临时文件 `calls3.txt` 不入库；样本 `target/fake-project` 也不入库，除非后续批准为测试 fixture。
