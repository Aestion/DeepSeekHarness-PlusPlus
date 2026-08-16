<#
.DESCRIPTION
Windows 代码签名脚本（signtool 参数化）。需要：
  - Windows SDK 的 signtool.exe（或 Windows Kits 安装）
  - 代码签名证书（PFX 或 证书存储）
用法：
  .\scripts\sign.ps1 -Path <exe> -Pfx <证书.pfx> -PfxPassword <密码> [-TimestampUrl http://timestamp.digicert.com]
  .\scripts\sign.ps1 -Path <exe> -Sha1 <指纹> -Store <My> [-TimestampUrl ...]
#>
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [string]$Pfx,
    [string]$PfxPassword,
    [string]$Sha1,
    [string]$Store = 'My',
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "文件不存在: $Path" }

# 定位 signtool
$signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
if (-not $signtool) {
    $kits = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Directory -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if ($kits) {
        $candidate = Join-Path $kits.FullName 'x64\signtool.exe'
        if (Test-Path $candidate) { $signtool = $candidate }
    }
}
if (-not $signtool) { throw '未找到 signtool.exe（需要 Windows SDK / Windows Kits）' }
Write-Host "[sign] signtool: $signtool"

$args = @('sign')
if ($Pfx) {
    if (-not (Test-Path -LiteralPath $Pfx -PathType Leaf)) { throw "PFX 不存在: $Pfx" }
    $args += @('/f', $Pfx)
    if ($PfxPassword) { $args += @('/p', $PfxPassword) }
} elseif ($Sha1) {
    $args += @('/sha1', $Sha1, '/s', $Store)
} else {
    throw '需要 -Pfx 或 -Sha1'
}
if ($TimestampUrl) { $args += @('/tr', $TimestampUrl, '/td', 'sha256') }
$args += @('/fd', 'sha256', '/v', $Path)

Write-Host "[sign] signtool $($args -join ' ')"
& $signtool @args
if ($LASTEXITCODE -ne 0) { throw "签名失败（exit $LASTEXITCODE）" }

# 验证签名
& $signtool verify /pa /v $Path | Out-Host
if ($LASTEXITCODE -ne 0) { Write-Warning '签名验证未通过（可能证书链/时间戳问题）' }
else { Write-Host '[sign] 签名验证通过' }
