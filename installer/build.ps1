param(
    [switch]$SkipCompress,
    [switch]$CopyToDesktop
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$falconExe = Join-Path $root "src-tauri\target\release\falconlauncher.exe"
$launcherExe = Join-Path $root "src-tauri\target\release\Glitchy Launcher.exe"
$launcherGz = Join-Path $PSScriptRoot "src-tauri\resources\launcher.gz"

if (Test-Path $falconExe) {
    Write-Output "Syncing latest build from $falconExe to $launcherExe..."
    Copy-Item -LiteralPath $falconExe -Destination $launcherExe -Force
}

if (-not (Test-Path $launcherExe)) {
    throw "Launcher executable not found at: $launcherExe. Please build the launcher first."
}

# Compress launcher executable into launcher.gz if not skipped or outdated
if (-not $SkipCompress -or -not (Test-Path $launcherGz)) {
    Write-Output "Compressing launcher executable to GZip ($launcherGz)..."
    if (Test-Path $launcherGz) { Remove-Item -LiteralPath $launcherGz -Force }
    $inStream = [System.IO.File]::OpenRead($launcherExe)
    $outStream = [System.IO.File]::Create($launcherGz)
    $gzStream = New-Object System.IO.Compression.GZipStream($outStream, [System.IO.Compression.CompressionLevel]::Optimal)
    $inStream.CopyTo($gzStream)
    $gzStream.Dispose()
    $outStream.Dispose()
    $inStream.Dispose()
    Write-Output "Compression complete: $((Get-Item $launcherGz).Length) bytes."
}

# Build the Web Setup Tauri application
Push-Location (Join-Path $PSScriptRoot "src-tauri")
try {
    Write-Output "Building Glitchy Web Setup (Tauri)..."
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}

$webSetupExe = Join-Path $PSScriptRoot "src-tauri\target\release\glitchy-web-setup.exe"
if (-not (Test-Path $webSetupExe)) {
    throw "Web Setup binary not found at $webSetupExe"
}

$out = Join-Path $PSScriptRoot "Glitchy Launcher WebSetup.exe"
Copy-Item -LiteralPath $webSetupExe -Destination $out -Force

# Sign executable if code signing certificate exists
$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -match "Glitchy" } | Select-Object -First 1
if ($cert) {
    Write-Output "Signing with certificate: $($cert.Subject)..."
    try {
        Set-AuthenticodeSignature -FilePath $out -Certificate $cert -TimestampServer "http://timestamp.digicert.com" -ErrorAction SilentlyContinue | Out-Null
    } catch {
        Set-AuthenticodeSignature -FilePath $out -Certificate $cert | Out-Null
    }
    try {
        Set-AuthenticodeSignature -FilePath $launcherExe -Certificate $cert -TimestampServer "http://timestamp.digicert.com" -ErrorAction SilentlyContinue | Out-Null
    } catch {
        Set-AuthenticodeSignature -FilePath $launcherExe -Certificate $cert | Out-Null
    }
}

# Update SHA256
$hash = Get-FileHash -Algorithm SHA256 -LiteralPath $out
$checksumPath = "$out.sha256"
Set-Content -LiteralPath $checksumPath -Encoding ascii -Value "$($hash.Hash)  $([IO.Path]::GetFileName($out))"

Write-Output "Built WebSetup successfully: $out"
Write-Output "SHA-256: $($hash.Hash)"

if ($CopyToDesktop) {
    $desktop = [Environment]::GetFolderPath("Desktop")
    Write-Output "Copying releases to Desktop: $desktop"
    Copy-Item -LiteralPath $out -Destination (Join-Path $desktop "Glitchy Launcher WebSetup.exe") -Force
    Copy-Item -LiteralPath $launcherExe -Destination (Join-Path $desktop "Glitchy Launcher.exe") -Force
    Write-Output "Copied 'Glitchy Launcher WebSetup.exe' and 'Glitchy Launcher.exe' to Desktop."

    $installedProg = Join-Path $env:LOCALAPPDATA "Programs\Glitchy Launcher\Glitchy Launcher.exe"
    if (Test-Path (Split-Path $installedProg -Parent)) {
        Copy-Item -LiteralPath $launcherExe -Destination $installedProg -Force
        Write-Output "Updated installed launcher at: $installedProg"
    }
}
