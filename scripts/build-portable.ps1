param(
    [switch]$SkipCompile,
    [switch]$SkipZip,
    [string]$McaSource = ''
)

$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
# 统一版本号：以根 package.json 为单一来源（PowerShell 解析，避免 node -p 的转义坑）。
$version = (Get-Content (Join-Path $workspace 'package.json') -Raw | ConvertFrom-Json).version
$artifactName = "DSHPlusPlus-$version-windows-x64"
$releaseRoot = Join-Path $workspace 'release'
$stage = Join-Path $releaseRoot $artifactName
$zip = Join-Path $releaseRoot "$artifactName.zip"
$desktopExe = Join-Path $workspace 'apps\desktop\src-tauri\target\release\DSHPlusPlus.exe'
# MCA sidecar：优先 -McaSource 显式指定；否则在环境变量 MCA_SIDECAR 或
# 常见位置查找（含 <workspace>\runtime\mca）。
if (-not $McaSource) { $McaSource = $env:MCA_SIDECAR }
$mcaSource = if ($McaSource -and (Test-Path -LiteralPath $McaSource -PathType Leaf)) {
    $McaSource
} else {
    $candidates = @(
        (Join-Path $workspace 'runtime\mca\mca-runtime.exe')
    )
    $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
}
$runtimePackage = Join-Path $workspace 'runtime\dsh'

function Assert-SafeReleaseTarget([string]$Target) {
    $resolvedRelease = [System.IO.Path]::GetFullPath($releaseRoot).TrimEnd('\')
    $resolvedTarget = [System.IO.Path]::GetFullPath($Target)
    if (-not $resolvedTarget.StartsWith($resolvedRelease + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace a path outside the release directory: $resolvedTarget"
    }
    if ([System.IO.Path]::GetFileName($resolvedTarget) -notlike 'DSHPlusPlus-*') {
        throw "Refusing to replace an unexpected release target: $resolvedTarget"
    }
}

Set-Location -LiteralPath $workspace
$env:CI = 'true'
pnpm install --force --no-frozen-lockfile --config.confirmModulesPurge=false
if ($LASTEXITCODE -ne 0) { throw 'Workspace dependency preparation failed.' }

pnpm pack:plugins
if ($LASTEXITCODE -ne 0) { throw 'Plugin packaging failed.' }

# Refresh tarball integrity and install the official DSH peer graph. DSH++'s
# internal peers are optional because all four packages are explicit runtime
# dependencies in runtime/dsh/package.json.
pnpm install --force --no-frozen-lockfile --config.confirmModulesPurge=false
if ($LASTEXITCODE -ne 0) { throw 'Workspace dependency installation failed.' }

if (-not $SkipCompile) {
    pnpm desktop:build
    if ($LASTEXITCODE -ne 0) { throw 'DSHPlusPlus.exe compilation failed.' }
}
if (-not (Test-Path -LiteralPath $desktopExe -PathType Leaf)) { throw "Desktop executable not found: $desktopExe" }
if (-not $mcaSource -or -not (Test-Path -LiteralPath $mcaSource -PathType Leaf)) { throw 'MCA sidecar not found; pass -McaSource <path>' }

New-Item -ItemType Directory -Force -Path $releaseRoot | Out-Null
Assert-SafeReleaseTarget $stage
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item -LiteralPath $desktopExe -Destination (Join-Path $stage 'DSHPlusPlus.exe')
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'runtime\node') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'runtime\mca') | Out-Null
$nodeExe = (Get-Command node -ErrorAction Stop).Source
Copy-Item -LiteralPath $nodeExe -Destination (Join-Path $stage 'runtime\node\node.exe')
Copy-Item -LiteralPath $mcaSource -Destination (Join-Path $stage 'runtime\mca\mca-runtime.exe')

# DSH++ browser gateway (CDP-managed Chrome + shared-tab bridge).
$browserSrc = Join-Path $workspace 'packages\browser-gateway'
$browserStage = Join-Path $stage 'runtime\browser'
New-Item -ItemType Directory -Force -Path $browserStage | Out-Null
Get-ChildItem -LiteralPath (Join-Path $browserSrc 'lib') -Filter '*.js' | ForEach-Object {
    $targetName = if ($_.Name -eq 'index.js') { 'gateway.js' } else { $_.Name }
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $browserStage $targetName)
}
Copy-Item -LiteralPath (Join-Path $browserSrc 'extension') -Destination (Join-Path $browserStage 'extension') -Recurse
New-Item -ItemType Directory -Force -Path (Join-Path $browserStage 'native-host') | Out-Null
Copy-Item -LiteralPath (Join-Path $browserSrc 'native-host\native-host.mjs') -Destination (Join-Path $browserStage 'native-host\native-host.mjs')
# 编译好的 native messaging host launcher（Chrome 直接启动 exe 最可靠）
$launcherExe = Join-Path $workspace 'tools\native-host-launcher\target\release\native-host-launcher.exe'
if (Test-Path -LiteralPath $launcherExe) {
    Copy-Item -LiteralPath $launcherExe -Destination (Join-Path $browserStage 'native-host-launcher.exe')
}

$deployTarget = Join-Path $stage 'runtime\dsh'
New-Item -ItemType Directory -Force -Path $deployTarget | Out-Null
# DSH++ 的代码库不携带 DSH 本体：运行时组装清单由本脚本在构建时生成
# （DSH 版本取自已提交的 compatibility.json，依赖从 npm registry 拉取）。
$compat = Get-Content (Join-Path $workspace 'runtime\manifests\compatibility.json') -Raw | ConvertFrom-Json
$dshVersion = $compat.deepseekHarness.publishedPackageBaseline
$portablePackage = @"
{
  "name": "dshplusplus-portable-runtime",
  "version": "$version",
  "private": true,
  "dependencies": {
    "@deepseek-ai/dsh": "$dshVersion"
  }
}
"@
[System.IO.File]::WriteAllText(
    (Join-Path $deployTarget 'package.json'),
    $portablePackage,
    (New-Object System.Text.UTF8Encoding($false))
)
$portableWorkspace = @"
packages:
  - .

nodeLinker: hoisted

allowBuilds:
  '@deepseek-ai/dsh-subprocess-local': true
  '@google/genai': true
  koffi: true
  node-pty: true
  protobufjs: true
"@
[System.IO.File]::WriteAllText(
    (Join-Path $deployTarget 'pnpm-workspace.yaml'),
    $portableWorkspace,
    (New-Object System.Text.UTF8Encoding($false))
)
$packs = @(
    (Join-Path $workspace '.tmp\packs\dshplusplus-multimodal-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-multimodal-llm-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-multimodal-router-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-bundle-plus-0.1.0-dev.1.tgz')
)
pnpm --dir $deployTarget add --save-prod --node-linker=hoisted "@deepseek-ai/dsh@$dshVersion" @packs
if ($LASTEXITCODE -ne 0) { throw 'Portable DSH runtime installation failed.' }

# Strip dev-only artifacts from the portable runtime. Node only executes
# .js/.mjs/.cjs at runtime, so declarations, source maps, and docs never load
# and only bloat the archive with very long paths that break stock extractors.
$runtimeNodeModules = Join-Path $deployTarget 'node_modules'
$prunePatterns = @('*.d.ts', '*.d.cts', '*.d.mts', '*.map', '*.ts', '*.cts', '*.mts', '*.md', '*.markdown', '*.pdb')
Get-ChildItem -LiteralPath $runtimeNodeModules -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
    foreach ($pattern in $prunePatterns) {
        if ($_.Name -like $pattern) { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue; break }
    }
}
# This portable build targets Windows x64 only: drop prebuilt binaries for
# other platforms (macOS, Linux, win32-arm64) that ship under prebuilds/.
Get-ChildItem -LiteralPath $runtimeNodeModules -Recurse -Directory -Filter 'prebuilds' -ErrorAction SilentlyContinue | ForEach-Object {
    $prebuildRoot = $_
    Get-ChildItem -LiteralPath $prebuildRoot.FullName -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -ne 'win32-x64' } |
        ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
}

Copy-Item -LiteralPath (Join-Path $workspace 'PORTABLE_README.md') -Destination (Join-Path $stage 'README.md')

if (-not $SkipZip) {
    Assert-SafeReleaseTarget $zip
    if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
    Compress-Archive -LiteralPath $stage -DestinationPath $zip -CompressionLevel Optimal
}

$stageBytes = (Get-ChildItem -LiteralPath $stage -Recurse -File | Measure-Object -Property Length -Sum).Sum
$result = [ordered]@{
    executable = (Join-Path $stage 'DSHPlusPlus.exe')
    directory = $stage
    zip = if ($SkipZip) { $null } else { $zip }
    unpackedBytes = $stageBytes
}
$result | ConvertTo-Json
