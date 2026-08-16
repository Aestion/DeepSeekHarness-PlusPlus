# 压缩便携发布目录（排除 .portable 运行数据）。
# 格式：auto（有 7-Zip 用 .7z 高压缩，否则 .zip）| zip | 7z
param(
    [ValidateSet('auto', 'zip', '7z')][string]$Format = 'auto',
    [string]$Stage = (Join-Path (Split-Path -Parent $PSScriptRoot) 'release')
)

$ErrorActionPreference = 'Stop'
$releaseRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $Stage)).TrimEnd('\')
$directoryName = Split-Path -Leaf $Stage
$source = Join-Path $releaseRoot $directoryName

if (-not (Test-Path -LiteralPath $source -PathType Container)) { throw "Portable directory not found: $source" }

function Find-7z {
    $candidates = @(
        (Get-Command 7z.exe, 7za.exe, 7zz.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source,
        'C:\Program Files\7-Zip\7z.exe',
        'C:\Program Files (x86)\7-Zip\7z.exe'
    )
    return ($candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } | Select-Object -First 1)
}

$sevenZip = Find-7z
$use7z = if ($Format -eq '7z') { $true }
    elseif ($Format -eq 'zip') { $false }
    else { $null -ne $sevenZip }

if ($use7z -and -not $sevenZip) { throw '请求 7z 格式但未找到 7-Zip' }
if ($use7z) {
    $target = Join-Path $releaseRoot "$directoryName.7z"
    $safe = [System.IO.Path]::GetFullPath($target)
    if ([System.IO.Path]::GetDirectoryName($safe) -ne $releaseRoot -or [System.IO.Path]::GetFileName($safe) -notlike 'DSHPlusPlus-*.7z') {
        throw "Unsafe 7z target: $safe"
    }
    if (Test-Path -LiteralPath $safe) { Remove-Item -LiteralPath $safe -Force }
    # -xr!.portable 排除运行数据（.dsh-home、浏览器数据、日志、证据）
    & $sevenZip a -t7z -mx=9 -mmt=on "-xr!.portable" $safe (Join-Path $releaseRoot $directoryName)
    if ($LASTEXITCODE -ne 0) { throw "7z failed with exit code $LASTEXITCODE" }
    $item = Get-Item -LiteralPath $safe
    [ordered]@{ format = '7z'; path = $item.FullName; bytes = $item.Length; mb = [Math]::Round($item.Length / 1MB, 1) } | ConvertTo-Json
    return
}

$zip = Join-Path $releaseRoot "$directoryName.zip"
$resolvedZip = [System.IO.Path]::GetFullPath($zip)
if ([System.IO.Path]::GetDirectoryName($resolvedZip) -ne $releaseRoot) { throw "Unsafe ZIP parent: $resolvedZip" }
if ([System.IO.Path]::GetFileName($resolvedZip) -notlike 'DSHPlusPlus-*.zip') { throw "Unsafe ZIP name: $resolvedZip" }
if (Test-Path -LiteralPath $resolvedZip) { Remove-Item -LiteralPath $resolvedZip -Force }

& 'C:\Windows\System32\tar.exe' -a -c -f $resolvedZip --exclude="$directoryName/.portable" -C $releaseRoot $directoryName
if ($LASTEXITCODE -ne 0) { throw "tar.exe failed with exit code $LASTEXITCODE" }

$item = Get-Item -LiteralPath $resolvedZip
[ordered]@{ format = 'zip'; path = $item.FullName; bytes = $item.Length; mb = [Math]::Round($item.Length / 1MB, 1) } | ConvertTo-Json
