# Isolated Windows CI only: installs, upgrades with the Rust SDK, and uninstalls.
[CmdletBinding()]
param([string]$Releases = "$PSScriptRoot/../dist/releases")
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:LOCALAPPDATA 'Mooz'
$config = Join-Path $env:APPDATA 'Mooz/config.json'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$testFeed = Join-Path $env:RUNNER_TEMP 'mooz-update-feed'
function Wait-Until([scriptblock]$Condition, [string]$Failure) {
    $end = (Get-Date).AddSeconds(45)
    while ((Get-Date) -lt $end) {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 300
    }
    throw $Failure
}
function Stop-Mooz { Get-Process Mooz -ErrorAction SilentlyContinue | Stop-Process -Force }
if (!$env:GITHUB_ACTIONS) { throw 'This smoke test runs only on disposable GitHub runners.' }
if (Test-Path $root) { throw 'Refusing to replace an existing installation.' }
try {
    $setup = (Resolve-Path "$Releases/Mooz-win-Setup.exe").Path
    $process = Start-Process -FilePath $setup -ArgumentList '--silent' -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "Install failed: $($process.ExitCode)" }
    Wait-Until { Test-Path "$root/current/Mooz.exe" } 'Installed executable missing.'
    # --silent intentionally suppresses the installer's normal first launch.
    Start-Process -FilePath "$root/current/Mooz.exe" | Out-Null
    Wait-Until { @(Get-Process Mooz -ErrorAction SilentlyContinue).Count -gt 0 } 'Mooz did not launch.'
    Stop-Mooz
    New-Item (Split-Path $config) -ItemType Directory -Force | Out-Null
    '{"sensitivity":1.7,"checkUpdates":false,"onboarded":true}' | Set-Content $config
    New-Item $runKey -Force | Out-Null
    New-ItemProperty $runKey -Name Mooz -Value ('"' + "$root/current/Mooz.exe" + '"') -PropertyType String -Force | Out-Null
    # Package the same exe with a higher fixture version. Never publish this feed.
    & "$PSScriptRoot/package.ps1" -Version '99.0.0' -OutputDir $testFeed
    cargo build --release --example update_smoke --locked
    if ($LASTEXITCODE -ne 0) { throw 'Smoke driver build failed.' }
    & "$PSScriptRoot/../target/release/examples/update_smoke.exe" $root $testFeed '99.0.0'
    if ($LASTEXITCODE -ne 0) { throw 'Rust update check/download/apply failed.' }
    Wait-Until { (Get-Content "$root/current/sq.version" -Raw) -match '<version>99.0.0</version>' } 'Update did not apply.'
    if ((Get-Content $config -Raw | ConvertFrom-Json).sensitivity -ne 1.7) { throw 'Update lost preferences.' }
    if (!(Get-ItemPropertyValue $runKey -Name Mooz)) { throw 'Update lost startup preference.' }
    $p = Start-Process "$root/current/Mooz.exe" -PassThru
    Start-Sleep -Seconds 2
    if ($p.HasExited) { throw 'Updated tray failed to launch.' }
    Stop-Mooz
    $p = Start-Process "$root/Update.exe" -ArgumentList '--silent uninstall' -PassThru -Wait
    if ($p.ExitCode -ne 0) { throw "Uninstall failed: $($p.ExitCode)" }
    Wait-Until { !(Test-Path "$root/current/Mooz.exe") } 'Uninstall left application files.'
    if (Get-ItemProperty $runKey -Name Mooz -ErrorAction SilentlyContinue) { throw 'Uninstall left startup registration.' }
    Write-Host 'OK: install, launch, SDK check/download/apply, settings/startup preservation, upgraded launch, uninstall.'
} finally {
    Stop-Mooz
}
