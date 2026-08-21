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

# DSH 本体不随完整包分发（由 DSHPlusPlus.exe 发现本地安装或引导用户安装）：
# exe 只自带 DSH++ 插件（plugins/@dshplusplus），materialize 时复制到 home profile。
$pluginsStage = Join-Path $stage 'plugins\@dshplusplus'
New-Item -ItemType Directory -Force -Path $pluginsStage | Out-Null
$packTarballs = @(
    (Join-Path $workspace '.tmp\packs\dshplusplus-multimodal-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-multimodal-llm-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-multimodal-router-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-tool-media-inspect-0.1.0-dev.1.tgz'),
    (Join-Path $workspace '.tmp\packs\dshplusplus-bundle-plus-0.1.0-dev.1.tgz')
)
$tmpExtract = Join-Path $workspace '.tmp\portable-plugins'
if (Test-Path -LiteralPath $tmpExtract) { Remove-Item -LiteralPath $tmpExtract -Recurse -Force }
New-Item -ItemType Directory -Force -Path $tmpExtract | Out-Null
foreach ($tarball in $packTarballs) {
    if (-not (Test-Path -LiteralPath $tarball -PathType Leaf)) { throw "缺少插件包: $tarball（先运行 pnpm pack:plugins）" }
    $target = Join-Path $tmpExtract ([System.IO.Path]::GetFileNameWithoutExtension($tarball))
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    # Windows tar（bsdtar）把 `E:\...` 的盘符冒号当成远端主机（"Cannot connect to E"），
    # 改成工作区相对路径即可避免：脚本顶部已 Set-Location $workspace。
    $relTarball = $tarball.Substring($workspace.Length).TrimStart('\')
    tar -xzf $relTarball -C $target
    if ($LASTEXITCODE -ne 0) { throw "解压插件包失败: $tarball" }
    $pkgDir = Join-Path $target 'package'
    if (-not (Test-Path -LiteralPath (Join-Path $pkgDir 'package.json') -PathType Leaf)) { throw "插件包结构异常: $tarball" }
    # 目标目录名 = package.json 的 name（@dshplusplus/multimodal → multimodal）
    $pkgName = (Get-Content (Join-Path $pkgDir 'package.json') -Raw | ConvertFrom-Json).name
    if (-not $pkgName) { throw "插件包缺少 name: $tarball" }
    $destName = $pkgName -replace '^@[^/]+/', ''
    Copy-Item -LiteralPath $pkgDir -Destination (Join-Path $pluginsStage $destName) -Recurse -Force
}
Remove-Item -LiteralPath $tmpExtract -Recurse -Force -ErrorAction SilentlyContinue

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
