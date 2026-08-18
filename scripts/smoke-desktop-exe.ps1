param(
    [string]$Stage = (Join-Path (Split-Path -Parent $PSScriptRoot) 'release'),
    [int]$Port = 18760
)

# 鍐掔儫娴嬭瘯锛氶獙璇?DSHPlusPlus.exe 鑳藉惎鍔ㄣ€丏SH 灏辩华銆侀€€鍑哄悗杩涚▼鏍戣娓呯悊銆?
# 璇存槑锛?
# - 涓荤獥鍙ｂ€滃叧闂€濈幇鍦ㄥ彧闅愯棌鍒版墭鐩橈紝鍥犳杩欓噷鐢?Stop-Process 妯℃嫙鈥滅湡姝ｉ€€鍑衡€?
#   锛堜笌鎵樼洏閫€鍑鸿蛋鍚屼竴鏉?Job Object 鍙ユ焺鍏抽棴璺緞锛孠ILL_ON_JOB_CLOSE 娓呯悊瀛愯繘绋嬫爲锛夈€?
# - 娴嬭瘯鍓?$Port 蹇呴』绌洪棽锛涘鏋滄湰鏈哄凡鏈?DSH++/DSH 鍦ㄨ繍琛岋紝璇锋敼鐢ㄧ┖闂茬鍙ｏ細
#   .\scripts\smoke-desktop-exe.ps1 -Port 18761 -Stage <鐩綍>
#   閰嶅悎 DSHPLUSPLUS_DATA_ROOT 鎸囧悜鐙珛鏁版嵁鐩綍锛屽彲鍋氬埌涓嶅共鎵扮幇鏈夊疄渚嬨€?
# - DSH 鏁版嵁榛樿浣跨敤鏍囧噯 home锛垀/.dsh锛夛紱鍐掔儫娴嬭瘯蹇呴』鐢?DSHPLUSPLUS_DSH_HOME
#   鎸囧悜闅旂鐩綍锛岄伩鍏嶆薄鏌撶湡瀹?DSH 鏁版嵁锛堜篃浼氳烦杩囦究鎼烘暟鎹縼绉伙級銆?

$ErrorActionPreference = 'Stop'
$Stage = [System.IO.Path]::GetFullPath($Stage)
$exe = Join-Path $Stage 'DSHPlusPlus.exe'
$app = $null
$mcaBefore = Get-NetTCPConnection -LocalPort 18767 -State Listen -ErrorAction SilentlyContinue
$smokeDshHome = Join-Path $Stage '.portable\smoke-dsh-home'

if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "Port $Port was occupied before the desktop smoke test."
}

try {
    # 棰勭疆鐙珛閰嶇疆锛氱鍙ｅ彇 $Port锛涘叧闂?MCA/娴忚鍣?澶氭ā鎬侊紝閬垮厤涓庣湡瀹炲疄渚?
    # 鐨?18767/18766 绔彛鍐茬獊锛屽啋鐑熷彧楠岃瘉 exe 鍚姩銆丏SH 灏辩华涓庤繘绋嬫爲娓呯悊銆?
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
    # PowerShell 5.1 鐨?Set-Content -Encoding UTF8 浼氬啓 BOM锛宻erde_json 鏃犳硶
    # 瑙ｆ瀽甯?BOM 鐨勯厤缃紙浼氶潤榛樺洖閫€榛樿绔彛锛夈€傚繀椤荤敤鏃?BOM 鐨?UTF-8 鍐欏叆銆?
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
        mcaPid = (Get-NetTCPConnection -LocalPort 18767 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty OwningProcess)
        embeddedWindow = $app.MainWindowTitle
    } | ConvertTo-Json
} finally {
    Remove-Item Env:DSHPLUSPLUS_AUTO_START -ErrorAction SilentlyContinue
    Remove-Item Env:DSHPLUSPLUS_DSH_HOME -ErrorAction SilentlyContinue
    if ($app -and -not $app.HasExited) {
        # 鍏抽棴涓荤獥鍙ｅ彧闅愯棌鍒版墭鐩橈紝鎵€浠ヨ繖閲岀洿鎺ュ己鏉€杩涚▼鏉ラ獙璇侀€€鍑烘竻鐞嗚矾寰勩€?
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        Wait-Process -Id $app.Id -Timeout 12 -ErrorAction SilentlyContinue
    }
}

Start-Sleep -Seconds 2
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "Managed DSH remained on port $Port after the desktop app closed."
}
if (-not $mcaBefore -and (Get-NetTCPConnection -LocalPort 18767 -State Listen -ErrorAction SilentlyContinue)) {
    throw 'Managed MCA remained after the desktop app closed.'
}
'PROCESS_CLEANUP_OK'
