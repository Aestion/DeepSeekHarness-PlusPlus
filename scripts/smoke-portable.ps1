param(
    [string]$Stage = (Join-Path (Split-Path -Parent $PSScriptRoot) 'release')
)

$ErrorActionPreference = 'Stop'
$mca = $null
$dsh = $null
$portable = Join-Path $Stage '.portable'
$mcaData = Join-Path $portable 'mca-smoke-data'
$mcaOut = Join-Path $portable 'mca-smoke.out.log'
$mcaErr = Join-Path $portable 'mca-smoke.err.log'
$dshOut = Join-Path $portable 'dsh-smoke.out.log'
$dshErr = Join-Path $portable 'dsh-smoke.err.log'

try {
    New-Item -ItemType Directory -Force -Path $mcaData | Out-Null
    $mca = Start-Process `
        -FilePath (Join-Path $Stage 'runtime\mca\mca-runtime.exe') `
        -ArgumentList @('serve', '--host', '127.0.0.1', '--port', '18765', '--data', $mcaData) `
        -WorkingDirectory $Stage `
        -WindowStyle Hidden `
        -RedirectStandardOutput $mcaOut `
        -RedirectStandardError $mcaErr `
        -PassThru

    $mcaReady = $false
    for ($index = 0; $index -lt 60; $index++) {
        try {
            $health = Invoke-RestMethod -Uri 'http://127.0.0.1:18765/api/health' -TimeoutSec 1
            if ($health.status -eq 'ok') { $mcaReady = $true; break }
        } catch {}
        Start-Sleep -Milliseconds 250
    }
    if (-not $mcaReady) { throw 'MCA did not become ready.' }

    $routeBody = @{
        mode = 'assist'
        capabilities = @('image', 'video', 'audio', 'document', 'web')
        capability_release_enabled = $false
        allow_external = $false
        model_provider = 'deepseek'
        model_family = 'deepseek'
        model_name = 'deepseek-chat'
        computer_allowed_risk = 'low'
        computer_require_confirmation = $true
        computer_access_mode = 'ask'
    } | ConvertTo-Json
    $route = Invoke-RestMethod `
        -Method Put `
        -Uri 'http://127.0.0.1:18765/api/agent-routes/deepseek-tui' `
        -ContentType 'application/json' `
        -Body $routeBody `
        -TimeoutSec 15

    $env:DSH_HOME = Join-Path $portable 'dsh-home'
    $env:DSH_TELEMETRY_DISABLED = '1'
    $dshArgs = @(
        (Join-Path $Stage 'runtime\dsh\node_modules\@deepseek-ai\dsh\lib\bin.js'),
        '--profile', 'dshplusplus', '--host', '127.0.0.1', '--port', '18760'
    )
    $dsh = Start-Process `
        -FilePath (Join-Path $Stage 'runtime\node\node.exe') `
        -ArgumentList $dshArgs `
        -WorkingDirectory $Stage `
        -WindowStyle Hidden `
        -RedirectStandardOutput $dshOut `
        -RedirectStandardError $dshErr `
        -PassThru

    $dshReady = $false
    for ($index = 0; $index -lt 100; $index++) {
        try {
            $response = Invoke-WebRequest -Uri 'http://127.0.0.1:18760/' -UseBasicParsing -TimeoutSec 1
            if ($response.StatusCode -eq 200) { $dshReady = $true; break }
        } catch {}
        if ($dsh.HasExited) { break }
        Start-Sleep -Milliseconds 250
    }

    [ordered]@{
        mcaReady = $mcaReady
        mcaContract = $health.contract
        routeAgent = $route.agent_id
        routeMode = $route.mode
        dshReady = $dshReady
        dshPid = $dsh.Id
        dshExited = $dsh.HasExited
        dshStatus = if ($dshReady) { $response.StatusCode } else { $null }
        dshErrorTail = (Get-Content -Tail 20 -LiteralPath $dshErr -ErrorAction SilentlyContinue) -join "`n"
    } | ConvertTo-Json -Depth 4
} finally {
    if ($dsh -and -not $dsh.HasExited) { Stop-Process -Id $dsh.Id -ErrorAction SilentlyContinue }
    if ($mca -and -not $mca.HasExited) { Stop-Process -Id $mca.Id -ErrorAction SilentlyContinue }
}
