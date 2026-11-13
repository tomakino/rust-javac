# Platform data (generated locally, checked in)

`internal/platform/17.0.19+10/` holds the platform data the compiler reads at
runtime. Both files are produced from the reference JDK and are **checked in**:

- `modules`: output of `cargo xtask gen-platform` (RPI3: method bodies removed,
  unreferenced constant pool entries and non-class resources dropped; tables,
  string pool and data blocks compressed), about 6.5 MiB.
- `ct.sym`: output of `cargo xtask gen-ct-sym` (RCS1: deduplicated entries plus
  block compression; entry order, names and contents are byte-identical to the
  JDK's `lib/ct.sym`), about 2.3 MiB.

Regenerate only when the reference JDK changes (needs JDK 17 and `JAVA_HOME`):

```powershell
cargo xtask gen-platform --out "internal\platform\17.0.19+10\modules"
cargo xtask gen-ct-sym --out "internal\platform\17.0.19+10\ct.sym"
```

`crates/rust-javac/src/main.rs` embeds both with `include_bytes!`, so the built
binary reads them from memory and needs no external files.

Verify that the checked-in images match a fresh generation:

```powershell
cargo xtask gen-platform --check
cargo xtask gen-ct-sym --check
```

Boundary: this is a Windows platform view. Public API code is portable; code
that references platform-internal classes (`sun.nio.fs.UnixFileSystemProvider`,
`sun.awt.X11.*`) must be regenerated on the target platform's JDK.

Source and license: the data comes from Eclipse Temurin 17.0.19+10 and is
licensed like the repository, GPL-2.0-only WITH Classpath-exception-2.0; see
`LICENSE` at the repository root.
