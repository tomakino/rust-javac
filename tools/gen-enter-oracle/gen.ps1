# Development-time ENTER oracle generator (baseline JDK 17.0.19).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$out = Join-Path $root "crates/javac-core/tests/data/enter-oracle.txt"
$classes = Join-Path $PSScriptRoot "out"
New-Item -ItemType Directory -Force -Path $classes | Out-Null
$exports = @(
  "--add-exports", "jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED",
  "--add-exports", "jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED",
  "--add-exports", "jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED",
  "--add-exports", "jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED",
  "--add-exports", "jdk.compiler/com.sun.tools.javac.tree=ALL-UNNAMED",
  "--add-exports", "jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED"
)
Push-Location $PSScriptRoot
try {
  & javac @exports -d $classes GenEnterOracle.java
  & java @exports -cp $classes GenEnterOracle | Set-Content -Encoding UTF8 $out
} finally {
  Pop-Location
}
Write-Output ("enter oracle written to " + $out)