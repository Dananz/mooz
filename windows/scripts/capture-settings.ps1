# Capture the real Windows renderer, using an isolated config and no input hooks.
$ErrorActionPreference = 'Stop'
if (!$env:GITHUB_ACTIONS) { throw 'Run on a disposable CI runner.' }
Add-Type -AssemblyName System.Drawing
$output = "$PSScriptRoot/../dist/settings-shots"
New-Item $output -ItemType Directory -Force | Out-Null
$env:MOOZ_CONFIG = Join-Path $env:RUNNER_TEMP 'mooz-settings-capture.json'
'{"checkUpdates":false,"onboarded":true}' | Set-Content $env:MOOZ_CONFIG
try {
    foreach ($state in @('general', 'shortcuts', 'behavior', 'applications', 'updates', 'about')) {
        $prefix = [IO.Path]::GetFullPath((Join-Path $output $state))
        $env:MOOZ_PREVIEW_SHOT = $prefix
        $env:MOOZ_PREVIEW_TAB = if ($state -in @('shortcuts','behavior')) { 'general' } else { $state }
        $env:MOOZ_PREVIEW_STATE = $state
        $app = Start-Process "$PSScriptRoot/../target/release/Mooz.exe" -ArgumentList '--settings' -PassThru
        if (!$app.WaitForExit(30000)) { $app.Kill(); throw "Settings capture timed out: $state" }
        if ($app.ExitCode -ne 0 -or !(Test-Path "$prefix.rgba")) { throw "Settings capture failed: $state ($($app.ExitCode))" }
        $size = (Get-Content "$prefix.size" -Raw).Trim().Split(' ')
        $width = [int]$size[0]; $height = [int]$size[1]
        $pixels = [IO.File]::ReadAllBytes("$prefix.rgba")
        for ($i=0; $i -lt $pixels.Length; $i+=4) { $red=$pixels[$i]; $pixels[$i]=$pixels[$i+2]; $pixels[$i+2]=$red }
        $bitmap = New-Object Drawing.Bitmap($width,$height,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $rect = New-Object Drawing.Rectangle(0,0,$width,$height)
            $data = $bitmap.LockBits($rect,[Drawing.Imaging.ImageLockMode]::WriteOnly,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try { [Runtime.InteropServices.Marshal]::Copy($pixels,0,$data.Scan0,$pixels.Length) } finally { $bitmap.UnlockBits($data) }
            $bitmap.Save("$prefix.png",[Drawing.Imaging.ImageFormat]::Png)
        } finally { $bitmap.Dispose() }
        Remove-Item "$prefix.rgba", "$prefix.size"
        Write-Host "Captured $state ($width x $height)"
    }
} finally {
    Remove-Item Env:MOOZ_CONFIG, Env:MOOZ_PREVIEW_SHOT, Env:MOOZ_PREVIEW_TAB, Env:MOOZ_PREVIEW_STATE -ErrorAction SilentlyContinue
}
