param(
    [string]$Stage = (Join-Path (Split-Path -Parent $PSScriptRoot) 'release'),
    [int]$Port = 18760
)

# 冒烟测试：验证 DSHPlusPlus.exe 能启动、DSH 就绪、退出后进程树被清理。
# 说明：
# - 主窗口“关闭”现在只隐藏到托盘，因此这里用 Stop-Process 模拟“真正退出”
#   （与托盘退出走同一条 Job Object 句柄关闭路径，KILL_ON_JOB_CLOSE 清理子进程树）。
# - 测试前 $Port 必须空闲；如果本机已有 DSH++/DSH 在运行，请改用空闲端口：
#   .\scripts\smoke-desktop-exe.ps1 -Port 18761 -Stage <目录>
#   配合 DSHPLUSPLUS_DATA_ROOT 指向独立数据目录，可做到不干扰现有实例。
# - DSH 数据默认使用标准 home（~/.dsh）；冒烟测试必须用 DSHPLUSPLUS_DSH_HOME
#   指向隔离目录，避免污染真实 DSH 数据（也会跳过便携数据迁移）。

$ErrorActionPreference = 'Stop'
$Stage = [System.IO.Path]::GetFullPath($Stage)
$exe = Join-Path $Stage 'DSHPlusPlus.exe'
$app = $null
$mcaBefore = Get-NetTCPConnection -LocalPort 18765 -State Listen -ErrorAction SilentlyContinue
$smokeDshHome = Join-Path $Stage '.portable\smoke-dsh-home'

if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "Port $Port was occupied before the desktop smoke test."
}

try {
    # 预置独立配置：端口取 $Port；关闭 MCA/浏览器/多模态，避免与真实实例
    # 的 18765/18766 端口冲突，冒烟只验证 exe 启动、DSH 就绪与进程树清理。
    $portable = Join-Path $Stage '.portable'
    New-Item -ItemType Directory -Force -Path $portable | Out-Null
    $smokeConfig = [ordered]@{
        dshHost = '127.0.0.1'
        dshPort = $Port
        workspace = $Stage
        autoStartDsh = $true
        autoOpenDshWindow = $false
        enableMca = $false
        enableBrowser = $false
        enableChromeUse = $false
        enableMultimodal = $false
    } | ConvertTo-Json
    # PowerShell 5.1 的 Set-Content -Encoding UTF8 会写 BOM，serde_json 无法
    # 解析带 BOM 的配置（会静默回退默认端口）。必须用无 BOM 的 UTF-8 写入。
    [System.IO.File]::WriteAllText(
        (Join-Path $portable 'dshplusplus.json'),
        $smokeConfig,
        (New-Object System.Text.UTF8Encoding($false))
    )

    $env:DSHPLUSPLUS_AUTO_START = '1'
    $env:DSHPLUSPLUS_DSH_HOME = $smokeDshHome
    $started = Get-Date
    $app = Start-Process `
        -FilePath $exe `
        -WorkingDirectory $Stage `
        -WindowStyle Hidden `
        -PassThru
    Remove-Item Env:DSHPLUSPLUS_AUTO_START -ErrorAction SilentlyContinue
    Remove-Item Env:DSHPLUSPLUS_DSH_HOME -ErrorAction SilentlyContinue

    $ready = $false
    for ($index = 0; $index -lt 160; $index++) {
        if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
            $ready = $true
            break
        }
        if ($app.HasExited) { break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $ready) { throw "DSH did not become ready on port $Port in 40 seconds." }

    $response = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/" -UseBasicParsing -TimeoutSec 5
    [ordered]@{
        appPid = $app.Id
        readySeconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 2)
        dshStatus = $response.StatusCode
        dshTitle = ([regex]::Match($response.Content, '<title>(.*?)</title>').Groups[1].Value)
        dshPid = (Get-NetTCPConnection -LocalPort $Port -State Listen | Select-Object -First 1 -ExpandProperty OwningProcess)
        mcaPid = (Get-NetTCPConnection -LocalPort 18765 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty OwningProcess)
        embeddedWindow = $app.MainWindowTitle
    } | ConvertTo-Json
} finally {
    Remove-Item Env:DSHPLUSPLUS_AUTO_START -ErrorAction SilentlyContinue
    Remove-Item Env:DSHPLUSPLUS_DSH_HOME -ErrorAction SilentlyContinue
    if ($app -and -not $app.HasExited) {
        # 关闭主窗口只隐藏到托盘，所以这里直接强杀进程来验证退出清理路径。
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        Wait-Process -Id $app.Id -Timeout 12 -ErrorAction SilentlyContinue
    }
}

Start-Sleep -Seconds 2
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "Managed DSH remained on port $Port after the desktop app closed."
}
if (-not $mcaBefore -and (Get-NetTCPConnection -LocalPort 18765 -State Listen -ErrorAction SilentlyContinue)) {
    throw 'Managed MCA remained after the desktop app closed.'
}
'PROCESS_CLEANUP_OK'
