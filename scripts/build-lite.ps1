# 构建 DSH++ Lite 插件包：给"已有 DSH"用户的几 MB 安装包。
# 产物：release/DSHPlusPlus-lite-<version>/ 与 .zip
# 注意：本文件包含中文，必须以 UTF-8 BOM 保存（PowerShell 5.1 按 ANSI 解析）。
param(
    [switch]$SkipPack
)

$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
# 统一版本号：以根 package.json 为单一来源（PowerShell 解析，避免 node -p 的转义坑）。
$version = (Get-Content (Join-Path $workspace 'package.json') -Raw | ConvertFrom-Json).version
$releaseRoot = Join-Path $workspace 'release'
$liteName = "DSHPlusPlus-lite-$version"
$liteDir = Join-Path $releaseRoot $liteName
$zip = Join-Path $releaseRoot "$liteName.zip"

if (-not $SkipPack) {
    pnpm pack:plugins
    if ($LASTEXITCODE -ne 0) { throw 'pack:plugins failed' }
}

if (Test-Path -LiteralPath $liteDir) { Remove-Item -LiteralPath $liteDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $liteDir | Out-Null

# 1) 插件 tarball
$packsDir = Join-Path $liteDir 'packs'
New-Item -ItemType Directory -Force -Path $packsDir | Out-Null
Copy-Item -Path (Join-Path $workspace '.tmp\packs\*.tgz') -Destination $packsDir

# 2) 零依赖安装器：用项目 tsc 把 scripts/install-plugins.ts 编译为单文件 ESM
#    （只依赖 node: 内置模块，无需 bundle）。
$tscBin = Join-Path $workspace 'node_modules\typescript\bin\tsc'
$tscOut = Join-Path $liteDir 'tmp'
& node $tscBin (Join-Path $workspace 'scripts\install-plugins.ts') --ignoreConfig --types node --module nodenext --moduleResolution nodenext --target es2022 --outDir $tscOut --skipLibCheck
if ($LASTEXITCODE -ne 0) { throw 'tsc compile failed' }
Copy-Item (Join-Path $tscOut 'install-plugins.js') (Join-Path $liteDir 'install.mjs') -Force
Remove-Item $tscOut -Recurse -Force

# 3) Windows 一键安装 cmd
$cmd = @'
@echo off
rem Install DSHPlusPlus plugins into your existing DSH.
rem Usage: install-to-dsh.cmd [--dsh-cli <path>] [--home <DSH_HOME>] [--profile <name>]
cd /d "%~dp0"
where node >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Node.js not found in PATH. Please install Node.js 22+ first.
    pause
    exit /b 1
)
node "%~dp0install.mjs" %*
if errorlevel 1 (
    echo [ERROR] Install failed. See messages above.
    pause
    exit /b 1
)
echo.
echo Done. Start DSH with:  dsh --profile dshplusplus
pause
'@
[System.IO.File]::WriteAllText(
    (Join-Path $liteDir '安装到已有DSH.cmd'),
    ($cmd -replace "`r?`n", "`r`n"),
    (New-Object System.Text.UTF8Encoding($false))
)

# 4) 使用说明
$readme = @"
# DSHPlusPlus Lite 插件包

为**已有 DeepSeek Harness** 的用户准备的几 MB 安装包：把 DSH++ 的五个插件装进你的
DSH Profile，获得多模态视觉、网页与浏览器能力，**不携带 Node/DSH/MCA 运行时**。

## 环境要求

- Node.js 22+（在 PATH 中）
- pnpm（npm install -g pnpm）
- 已安装 DeepSeek Harness（dsh 命令在 PATH 中，或安装时用参数指定）

## 安装

双击运行「安装到已有DSH.cmd」，或手动执行：

    node install.mjs

可用参数：

    --dsh-cli <path>   dsh CLI 路径（bin.js 或可执行文件）；默认按
                       DSHPLUSPLUS_DSH_CLI 环境变量 → PATH 中的 dsh 查找
    --home <path>      DSH_HOME（默认 \$DSH_HOME 或 ~/.dsh）
    --profile <name>   目标 profile（默认 dshplusplus）
    --packs-dir <dir>  插件包目录（默认本包内 packs/）

## 使用

    dsh --profile dshplusplus

## 说明

- 插件安装会把声明 bundle 的 @dshplusplus/bundle-plus 自动加入 Profile 层栈；
- 首次运行请在 DSH 的设置中配置主模型与（可选的）视觉模型；
- 完整便携版（含运行时，约 190MB）见仓库 Release 的 DSHPlusPlus-*-windows-x64.zip。
"@
[System.IO.File]::WriteAllText(
    (Join-Path $liteDir '使用说明.md'),
    $readme,
    (New-Object System.Text.UTF8Encoding($false))
)

# 5) 打 zip
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $liteDir '*') -DestinationPath $zip -CompressionLevel Optimal

$size = (Get-ChildItem -LiteralPath $liteDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$zipSize = (Get-Item -LiteralPath $zip).Length
[ordered]@{
    dir = $liteDir
    zip = $zip
    unpackedMB = [Math]::Round($size / 1MB, 2)
    zipMB = [Math]::Round($zipSize / 1MB, 2)
} | ConvertTo-Json
