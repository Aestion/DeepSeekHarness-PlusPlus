# 生成远程更新清单 update-manifest.json（发布到 GitHub Release 时上传）。
# 控制中心的"检查更新"读取该清单（更新源 URL 指向它）。
# 用法：
#   powershell -File scripts\build-manifest.ps1 [-Tag v0.1.0-dev.1] [-McaVersion 1.0.0] [-McaUrl https://…/mca-runtime.exe]
param(
    [string]$Tag = '',
    [string]$McaVersion = '',
    [string]$McaUrl = ''
)

$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$version = (Get-Content (Join-Path $workspace 'package.json') -Raw | ConvertFrom-Json).version
if (-not $Tag) { $Tag = "v$version" }
$base = "https://github.com/Aestion/DeepSeekHarness-PlusPlus/releases/download/$Tag"

$manifest = [ordered]@{
    app = [ordered]@{
        version = $version
        url     = "$base/DSHPlusPlus.update.exe"
    }
    plugins = [ordered]@{
        urlPrefix = "$base/"
        packages  = [ordered]@{}
    }
}

# 各插件包版本（packages/*/package.json 单一来源）
foreach ($pkg in @('multimodal', 'multimodal-llm', 'multimodal-router', 'tool-media-inspect', 'bundle-plus')) {
    $pkgVersion = (Get-Content (Join-Path $workspace "packages\$pkg\package.json") -Raw | ConvertFrom-Json).version
    $manifest.plugins.packages[$pkg] = $pkgVersion
}

if ($McaVersion -and $McaUrl) {
    $manifest.mca = [ordered]@{
        version = $McaVersion
        url     = $McaUrl
    }
}

$out = Join-Path $workspace 'release\update-manifest.json'
[System.IO.File]::WriteAllText(
    $out,
    ($manifest | ConvertTo-Json -Depth 5),
    (New-Object System.Text.UTF8Encoding($false))
)
Write-Host "已生成: $out"
Get-Content $out
