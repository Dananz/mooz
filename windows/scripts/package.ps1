# Run on Windows after cargo build --release. vpk and the Rust SDK are pinned.
[CmdletBinding()]
param(
    [string]$Version = (Get-Content "$PSScriptRoot/../../VERSION" -Raw).Trim(),
    [string]$OutputDir = "$PSScriptRoot/../dist/releases",
    [string]$PackDir = "$PSScriptRoot/../target/release",
    [switch]$RequireSignature
)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Version must be major.minor.patch.' }
if (!(Test-Path "$PackDir/Mooz.exe")) { throw 'Build Mooz.exe before packaging.' }
$stage = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
$certificate = $null
try {
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item "$PackDir/Mooz.exe" $stage
    # Stage only the application, never the compiler output or smoke-test tools.
    Copy-Item "$PSScriptRoot/../../LICENSE" $stage
    $vpkArgs = @('pack', '--packId', 'Mooz', '--packVersion', $Version,
        '--packDir', $stage, '--mainExe', 'Mooz.exe', '--packTitle', 'Mooz',
        '--packAuthors', 'Tomer Danan', '--icon', "$PSScriptRoot/../assets/mooz.ico",
        '--runtime', 'win-x64', '--channel', 'win', '--shortcuts', 'StartMenuRoot',
        '--outputDir', $OutputDir)
    if ($env:WINDOWS_SIGNING_PFX_BASE64 -and $env:WINDOWS_SIGNING_PFX_PASSWORD) {
        $pfx = Join-Path $stage 'signing.pfx'
        [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_SIGNING_PFX_BASE64))
        $password = ConvertTo-SecureString $env:WINDOWS_SIGNING_PFX_PASSWORD -AsPlainText -Force
        $certificate = Import-PfxCertificate -FilePath $pfx -CertStoreLocation Cert:\CurrentUser\My -Password $password
        Remove-Item $pfx
        $vpkArgs += @('--signParams', "/sha1 $($certificate.Thumbprint) /s My /fd SHA256 /tr http://timestamp.digicert.com /td SHA256")
    } elseif ($RequireSignature) {
        throw 'Public releases require WINDOWS_SIGNING_PFX_BASE64 and WINDOWS_SIGNING_PFX_PASSWORD repository secrets.'
    } else {
        Write-Warning 'Unsigned test build. Public releases require a trusted code-signing certificate.'
    }
    & vpk @vpkArgs
    if ($LASTEXITCODE -ne 0) { throw "vpk pack failed: $LASTEXITCODE" }
    $installer = Join-Path $OutputDir 'Mooz-win-Setup.exe'
    if (!(Test-Path $installer)) { throw 'Installer missing.' }
    if (!(Test-Path (Join-Path $OutputDir 'releases.win.json'))) { throw 'Update feed missing.' }
    if ($RequireSignature -and (Get-AuthenticodeSignature $installer).Status -ne 'Valid') {
        throw 'Installer Authenticode signature is not valid.'
    }
    $checksums = Get-ChildItem $OutputDir -File | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
        "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
    }
    [IO.File]::WriteAllText((Join-Path $OutputDir 'SHA256SUMS.txt'), (($checksums -join "`n") + "`n"), [Text.Encoding]::ASCII)
} finally {
    if ($certificate) { Remove-Item "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -ErrorAction SilentlyContinue }
    Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
}
