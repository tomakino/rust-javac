# Dev-time fixture generator (baseline JDK 17 javac).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$out = Join-Path $root "crates/javac-core/tests/data/fixtures"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Get-ChildItem $out -File -ErrorAction SilentlyContinue | Remove-Item -Force
Push-Location (Join-Path $PSScriptRoot "src")
try {
    javac -version
    javac -parameters --release 17 -d $out Fixtures.java RecordFixture.java SealedFixture.java AnnotatedFixture.java
    javac -parameters --release 17 -d $out module-info.java fixtures/p/Service.java fixtures/p/ServiceImpl.java
} finally {
    Pop-Location
}
Write-Output ("fixtures written to " + $out)
Get-ChildItem $out -Recurse -File | Select-Object -ExpandProperty FullName