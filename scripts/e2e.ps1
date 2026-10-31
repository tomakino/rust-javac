#!/usr/bin/env pwsh
# rust-javac end-to-end checks (L3 project bytes, L4 CLI behavior, L5 perf).
#
# Usage:
#   pwsh ./scripts/e2e.ps1                 # Gson bytes + CLI checks
#   pwsh ./scripts/e2e.ps1 -IncludeOkhttp  # also compare the okhttp classes
#   pwsh ./scripts/e2e.ps1 -Perf           # also run the release benchmark
param(
    [string]$JdkHome = $env:JAVA_HOME,
    [string]$Repo = (Split-Path -Parent $PSScriptRoot),
    [string]$DebugBinary = '',
    [string]$ReleaseBinary = '',
    [string]$GsonSources = '',
    [string]$OkhttpRoot = '',
    [switch]$IncludeOkhttp,
    [switch]$Perf
)
$ErrorActionPreference = 'Stop'
$javac = Join-Path $JdkHome 'bin\javac.exe'
if (-not (Test-Path $javac)) { throw "javac not found: $javac" }
if ($DebugBinary -eq '') { $DebugBinary = Join-Path $Repo 'target\debug\rust-javac.exe' }
if ($ReleaseBinary -eq '') { $ReleaseBinary = Join-Path $Repo 'target\release\rust-javac.exe' }
if (-not (Test-Path $DebugBinary)) { throw "rust-javac (debug) not found: $DebugBinary" }
if ($GsonSources -eq '') { $GsonSources = Join-Path (Split-Path -Parent $Repo) 'javac-testdata\gson-crosscheck\sources-all.txt' }
if ($OkhttpRoot -eq '') { $OkhttpRoot = Join-Path (Split-Path -Parent $Repo) 'javac-testdata\okhttp-parent-3.14.9' }
$results = New-Object System.Collections.Generic.List[object]
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results.Add([pscustomobject]@{ Check = $name; Ok = $ok; Detail = $detail })
    $status = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Output ("[{0}] {1}{2}" -f $status, $name, $(if ($detail -ne '') { " - $detail" } else { '' }))
}
$tmp = Join-Path $env:TEMP ('e2e-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $tmp | Out-Null
function Compile-Dir([string]$exe, [string]$sourcesFile, [string]$outDir, [string]$log) {
    $srcs = Get-Content $sourcesFile
    $argsFile = Join-Path $tmp ((Split-Path -Leaf $log) + '.args')
    Set-Content $argsFile (@('-d', $outDir) + $srcs) -Encoding utf8
    & $exe '-J-Duser.language=en' '-J-Duser.country=US' ('@' + $argsFile) *> $log
    return $LASTEXITCODE
}
# ---- L3: Gson bytes ----
if (Test-Path $GsonSources) {
    $jOut = Join-Path $tmp 'gson-j'
    $rOut = Join-Path $tmp 'gson-r'
    $je = Compile-Dir $javac $GsonSources $jOut (Join-Path $tmp 'gson-j.log')
    $re = Compile-Dir $DebugBinary $GsonSources $rOut (Join-Path $tmp 'gson-r.log')
    $diff = 0
    $classes = @(Get-ChildItem $rOut -Recurse -Filter '*.class' -ErrorAction SilentlyContinue)
    foreach ($c in $classes) {
        $rel = $c.FullName.Substring($rOut.Length + 1)
        $jf = Join-Path $jOut $rel
        if (-not (Test-Path $jf)) { $diff++; continue }
        if ((Get-FileHash $c.FullName -Algorithm SHA256).Hash -ne (Get-FileHash $jf -Algorithm SHA256).Hash) { $diff++ }
    }
    $diag = ''
    $jd = Get-Content (Join-Path $tmp 'gson-j.log') -ErrorAction SilentlyContinue
    $rd = Get-Content (Join-Path $tmp 'gson-r.log') -ErrorAction SilentlyContinue
    if (Compare-Object $jd $rd) { $diag = 'diagnostics differ' }
    Record 'L3 Gson bytes' (($je -eq 0) -and ($re -eq 0) -and ($diff -eq 0) -and ($diag -eq '')) ("classes={0} sha_diff={1} {2}" -f $classes.Count, $diff, $diag)
} else {
    Record 'L3 Gson bytes' $false 'sources-all.txt not found'
}
# ---- L3: okhttp bytes (optional) ----
if ($IncludeOkhttp) {
    $okRoot = $OkhttpRoot
    $files = New-Object System.Collections.Generic.List[string]
    foreach ($m in @('okhttp/src/main/java','okhttp-generated','okhttp-testing-support/src/main/java','okhttp-tls/src/main/java','mockwebserver/src/main/java','okhttp-urlconnection/src/main/java','okhttp-logging-interceptor/src/main/java','okhttp-tests/src/test/java','mockwebserver/src/test/java','okhttp-tls/src/test/java','okhttp-urlconnection/src/test/java')) {
        $d = Join-Path $okRoot $m
        if (Test-Path $d) { Get-ChildItem $d -Recurse -Filter '*.java' | Where-Object { $_.Name -ne 'module-info.java' } | ForEach-Object { $files.Add($_.FullName) } }
    }
    $files = @($files | Sort-Object -Unique)
    $deps = Join-Path (Split-Path -Parent $Repo) 'javac-testdata\okhttp-deps'
    $cp = (@(Get-ChildItem $deps -Filter '*.jar' | ForEach-Object FullName) -join ';')
    $jOut = Join-Path $tmp 'ok-j'; $rOut = Join-Path $tmp 'ok-r'
    $argsFile = Join-Path $tmp 'ok.args'; Set-Content $argsFile (@('-cp', $cp) + $files) -Encoding utf8
    & $javac '-J-Duser.language=en' '-J-Duser.country=US' '-d' $jOut ('@' + $argsFile) *> (Join-Path $tmp 'ok-j.log'); $je = $LASTEXITCODE
    & $DebugBinary '-J-Duser.language=en' '-J-Duser.country=US' '-d' $rOut ('@' + $argsFile) *> (Join-Path $tmp 'ok-r.log'); $re = $LASTEXITCODE
    $diff = 0
    $classes = @(Get-ChildItem $rOut -Recurse -Filter '*.class' -ErrorAction SilentlyContinue)
    foreach ($c in $classes) {
        $rel = $c.FullName.Substring($rOut.Length + 1)
        $jf = Join-Path $jOut $rel
        if (-not (Test-Path $jf)) { $diff++; continue }
        if ((Get-FileHash $c.FullName -Algorithm SHA256).Hash -ne (Get-FileHash $jf -Algorithm SHA256).Hash) { $diff++ }
    }
    Record 'L3 okhttp bytes' (($je -eq 0) -and ($re -eq 0) -and ($diff -eq 0)) ("classes={0} sha_diff={1}" -f $classes.Count, $diff)
}
# ---- L4: CLI behavior ----
$hl = Join-Path $tmp 'help-lint.txt'
& $javac '-J-Duser.language=en' '-J-Duser.country=US' '--help-lint' *> (Join-Path $tmp 'help-lint-j.txt')
& $DebugBinary '-J-Duser.language=en' '-J-Duser.country=US' '--help-lint' *> $hl
$hlSame = -not (Compare-Object (Get-Content (Join-Path $tmp 'help-lint-j.txt')) (Get-Content $hl))
Record 'L4 --help-lint' $hlSame
$xsrc = Join-Path $tmp 'X.java'
"class X { void f() { int i = 0; } }" | Set-Content $xsrc
& $javac '-J-Duser.language=en' '-J-Duser.country=US' '-Xstdout' (Join-Path $tmp 'xs-j.txt') '-d' (Join-Path $tmp 'xs-j') $xsrc *> $null
& $DebugBinary '-J-Duser.language=en' '-J-Duser.country=US' '-Xstdout' (Join-Path $tmp 'xs-r.txt') '-d' (Join-Path $tmp 'xs-r') $xsrc *> (Join-Path $tmp 'xs-r-console.txt')
$xsSame = ((Get-FileHash (Join-Path $tmp 'xs-j.txt') -Algorithm SHA256).Hash -eq (Get-FileHash (Join-Path $tmp 'xs-r.txt') -Algorithm SHA256).Hash) -and ([string]::IsNullOrWhiteSpace((Get-Content (Join-Path $tmp 'xs-r-console.txt') -Raw -ErrorAction SilentlyContinue)))
Record 'L4 -Xstdout' $xsSame
& $DebugBinary '-J-Duser.language=en' '-J-Duser.country=US' '--help' *> (Join-Path $tmp 'help.txt')
$jLine = @(Get-Content (Join-Path $tmp 'help.txt') | Select-String '^\s+-J<flag>').Count
Record 'L4 --help -J line' ($jLine -eq 1)
& $DebugBinary *> (Join-Path $tmp 'noargs.txt'); $noargsExit = $LASTEXITCODE
Record 'L4 no-arguments exit' ($noargsExit -eq 2) ("exit=$noargsExit")
& $DebugBinary '--not-an-option' *> (Join-Path $tmp 'badopt.txt'); $badExit = $LASTEXITCODE
Record 'L4 invalid option exit' ($badExit -eq 2) ("exit=$badExit")
& $DebugBinary '-J-Duser.language=en' '-J-Duser.country=US' '--version' *> (Join-Path $tmp 'ver.txt')
$ver = ((Get-Content (Join-Path $tmp 'ver.txt') -Raw) -replace '\s+', ' ').Trim()
Record 'L4 --version' ($ver -eq 'javac 17.0.19') $ver
# ---- L5: perf (release) ----
if ($Perf -and (Test-Path $ReleaseBinary)) {
    $times = @()
    for ($i = 1; $i -le 5; $i++) {
        $out = Join-Path $tmp ("perf" + $i)
        New-Item -ItemType Directory -Force $out | Out-Null
        $sw = [Diagnostics.Stopwatch]::StartNew()
        & $ReleaseBinary '-J-Duser.language=en' '-J-Duser.country=US' '-d' $out ('@' + (Join-Path $tmp 'gson-j.log.args')) *> $null
        $sw.Stop()
        $times += [math]::Round($sw.Elapsed.TotalSeconds, 3)
    }
    $median = ($times | Sort-Object)[2]
    Record 'L5 Gson release' $true ("runs={0} median={1}s" -f ($times -join ','), $median)
}
$failed = @($results | Where-Object { -not $_.Ok }).Count
Write-Output ""
Write-Output ("E2E summary: {0}/{1} passed" -f ($results.Count - $failed), $results.Count)
if ($failed -gt 0) { exit 1 } else { exit 0 }
