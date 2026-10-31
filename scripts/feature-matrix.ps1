#!/usr/bin/env pwsh
# rust-javac feature matrix check (Java 9-17 language / classfile features).
#
# Every unit under internal/feature-matrix/<release>/<feature>/ is compiled by
# the official javac and by rust-javac with the same arguments; the produced
# .class files are compared by relative path and SHA-256, and the diagnostics
# and exit codes must match.
#
# Usage:
#   pwsh ./scripts/feature-matrix.ps1
#   pwsh ./scripts/feature-matrix.ps1 -EnablePreview
param(
    [string]$JdkHome = $env:JAVA_HOME,
    [string]$Repo = (Split-Path -Parent $PSScriptRoot),
    [string]$DebugBinary = '',
    [string]$MatrixRoot = '',
    [switch]$EnablePreview
)
$ErrorActionPreference = 'Stop'
$javac = Join-Path $JdkHome 'bin\javac.exe'
if (-not (Test-Path $javac)) { throw "javac not found: $javac" }
if ($DebugBinary -eq '') { $DebugBinary = Join-Path $Repo 'target\debug\rust-javac.exe' }
if (-not (Test-Path $DebugBinary)) { throw "rust-javac (debug) not found: $DebugBinary" }
if ($MatrixRoot -eq '') { $MatrixRoot = Join-Path $Repo 'testdata\feature-matrix' }
if (-not (Test-Path $MatrixRoot)) { throw "matrix root not found: $MatrixRoot" }

# Keep the scratch directory inside the workspace: the DSH file sandbox lets
# child processes (javac/rust-javac) write only under the repository.
$tmp = Join-Path $Repo ('target\feature-matrix-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $tmp | Out-Null
$results = New-Object System.Collections.Generic.List[object]
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results.Add([pscustomobject]@{ Check = $name; Ok = $ok; Detail = $detail })
    $status = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Output ("[{0}] {1}{2}" -f $status, $name, $(if ($detail -ne '') { " - $detail" } else { '' }))
}

$units = New-Object System.Collections.Generic.List[object]
foreach ($release in Get-ChildItem $MatrixRoot -Directory | Sort-Object Name) {
    foreach ($feature in Get-ChildItem $release.FullName -Directory | Sort-Object Name) {
        $sources = @(Get-ChildItem $feature.FullName -Recurse -Filter '*.java' -ErrorAction SilentlyContinue |
            ForEach-Object { $_.FullName } | Sort-Object)
        if ($sources.Count -eq 0) { continue }
        $isModule = @($sources | Where-Object { $_ -match 'module-info\.java$' }).Count -gt 0
        $units.Add([pscustomobject]@{
            Name = "$($release.Name)/$($feature.Name)"
            Dir = $feature.FullName
            Sources = $sources
            IsModule = $isModule
        })
    }
}
if ($units.Count -eq 0) { throw "no units found under $MatrixRoot" }
Write-Output "units=$($units.Count) preview=$($EnablePreview.IsPresent) tmp=$tmp"

function Compile-Unit {
    param([string]$exe, $unit, [string]$outDir, [string]$log)
    $argsList = @('-J-Duser.language=en', '-J-Duser.country=US')
    if ($EnablePreview) { $argsList += @('--enable-preview', '-source', '17') }
    if ($unit.IsModule) { $argsList += @('--module-source-path', (Join-Path $unit.Dir 'src')) }
    $argsList += @('-d', $outDir)
    $argsList += $unit.Sources
    & $exe @argsList 1> $log 2>&1
    return $LASTEXITCODE
}

function Read-Log([string]$path) {
    if (-not (Test-Path $path)) { return ,@() }
    $lines = @(Get-Content $path | ForEach-Object { ($_ -replace "\r", '') -replace [regex]::Escape($tmp), '<tmp>' })
    return ,$lines
}

$failures = 0
foreach ($unit in $units) {
    $safe = ($unit.Name -replace '[\\/]', '-')
    $jLog = Join-Path $tmp ($safe + '.javac.log')
    $rLog = Join-Path $tmp ($safe + '.rust.log')
    $jOut = Join-Path $tmp ($safe + '.javac-out')
    $rOut = Join-Path $tmp ($safe + '.rust-out')
    New-Item -ItemType Directory -Force $jOut, $rOut | Out-Null

    $je = Compile-Unit $javac $unit $jOut $jLog
    $re = Compile-Unit $DebugBinary $unit $rOut $rLog

    $jDiag = @(Read-Log $jLog)
    $rDiag = @(Read-Log $rLog)
    $diagDiff = if ($jDiag.Count -eq 0 -and $rDiag.Count -eq 0) {
        0
    } else {
        @(Compare-Object -ReferenceObject @($jDiag) -DifferenceObject @($rDiag) -SyncWindow 0).Count
    }

    $jClasses = @(Get-ChildItem $jOut -Recurse -Filter '*.class' -ErrorAction SilentlyContinue)
    $rClasses = @(Get-ChildItem $rOut -Recurse -Filter '*.class' -ErrorAction SilentlyContinue)
    $missing = 0
    $shaDiff = 0
    foreach ($c in $rClasses) {
        $rel = $c.FullName.Substring($rOut.Length + 1)
        $jf = Join-Path $jOut $rel
        if (-not (Test-Path $jf)) { $missing++; continue }
        if ((Get-FileHash $c.FullName -Algorithm SHA256).Hash -ne (Get-FileHash $jf -Algorithm SHA256).Hash) { $shaDiff++ }
    }
    $extra = $jClasses.Count - $rClasses.Count + $missing
    $ok = ($je -eq 0) -and ($re -eq 0) -and ($diagDiff -eq 0) -and ($missing -eq 0) -and ($shaDiff -eq 0)
    $detail = "classes={0} sha_diff={1} missing={2} diag_diff={3} exit={4}/{5}" -f $rClasses.Count, $shaDiff, $missing, $diagDiff, $je, $re
    if (-not $ok) {
        $failures++
        if ($rDiag.Count -gt 0) { Write-Output ("  rust diagnostics:"); $rDiag | Select-Object -First 8 | ForEach-Object { Write-Output ("    " + $_) } }
        if ($jDiag.Count -gt 0) { Write-Output ("  javac diagnostics:"); $jDiag | Select-Object -First 8 | ForEach-Object { Write-Output ("    " + $_) } }
    }
    Record $unit.Name $ok $detail
}

Write-Output ""
Write-Output ("Feature matrix summary: {0}/{1} passed, {2} failed" -f ($units.Count - $failures), $units.Count, $failures)
if ($failures -gt 0) { exit 1 }
exit 0
