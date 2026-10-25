# Class-file fixtures generator

Development-time only: compiles the Java samples below with the baseline JDK 17
javac and stores the resulting .class files under
`crates/javac-core/tests/data/fixtures/` for the ClassReader tests. Tests never
invoke a JVM at runtime; they read the checked-in .class bytes.

Generation command (baseline: Eclipse Adoptium JDK 17.0.19, `javac -version` =
javac 17.0.19):

```powershell
pwsh tools/gen-class-fixtures/gen.ps1
```

The fixtures are simple original samples (no third-party code); they are
licensed under GPLv2+CE, matching the repository's reference baseline.

## Fixtures (compiled at development time, checked in as .class):

- Fixtures.class / Fixtures$Member.class / Fixtures$Nested.class / Fixtures$1Local.class / Fixtures$1.class
  (inner classes, EnclosingMethod, ConstantValue, SourceFile);
- RecordFixture.class (Record attribute, record components/accessors);
- SealedFixture.class / Circle.class / Square.class (PermittedSubclasses, sealed);
- AnnotatedFixture.class / MyAnno.class (Runtime[In]visibleAnnotations, AnnotationDefault);
- TypeAnnotated.class / MyTypeAnno.class (RuntimeVisibleTypeAnnotations; TYPE_USE);
- module-info.class + fixtures/p/{Service,ServiceImpl}.class (Module/ModuleResolution directives).

The tests read these bytes directly; no JVM is invoked at test runtime.