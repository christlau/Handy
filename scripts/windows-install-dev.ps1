#Requires -Version 5.1
<#
.SYNOPSIS
    Build and install a local dev copy of Handy on Windows.

.DESCRIPTION
    1. Builds a release binary without installer/signing:
         bun run tauri build --no-bundle
    2. Kills any running handy.exe instance.
    3. Copies the compiled binary to:
         %LOCALAPPDATA%\Programs\Handy-Dev\handy.exe
    4. Launches the newly installed binary.

    Run this script from the repo root whenever you want to update your local
    dev install.  Does NOT require admin rights.

.NOTES
    CARGO_TARGET_DIR must be set (windows-setup.ps1 sets it to C:\h).
    If it is not set, the binary is found at src-tauri\target\release\handy.exe.
#>

# ── Helpers ──────────────────────────────────────────────────────────────────

function Write-Step {
    param([string]$Message)
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host "  ✓ $Message" -ForegroundColor Green
}

function Write-Warning-Custom {
    param([string]$Message)
    Write-Host "  ⚠ $Message" -ForegroundColor Yellow
}

function Write-Info {
    param([string]$Message)
    Write-Host "  · $Message" -ForegroundColor Gray
}

function Write-Err {
    param([string]$Message)
    Write-Host "  ✗ $Message" -ForegroundColor Red
}

# ── Resolve paths ─────────────────────────────────────────────────────────────

$repoRoot   = Split-Path -Parent $PSScriptRoot
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\Handy-Dev'
$installExe = Join-Path $installDir 'handy.exe'

# Determine where the compiled binary will land.
# If CARGO_TARGET_DIR is set (e.g. C:\h), artifacts are there.
# Otherwise they live in the repo's src-tauri\target\.
if ($env:CARGO_TARGET_DIR) {
    $builtExe = Join-Path $env:CARGO_TARGET_DIR 'release\handy.exe'
    Write-Info "CARGO_TARGET_DIR is set — expecting binary at: $builtExe"
} else {
    $builtExe = Join-Path $repoRoot 'src-tauri\target\release\handy.exe'
    Write-Warning-Custom "CARGO_TARGET_DIR is not set. Run windows-setup.ps1 first to avoid MAX_PATH issues."
    Write-Info "Expecting binary at: $builtExe"
}

# ── Verify repo root ──────────────────────────────────────────────────────────

if (-not (Test-Path (Join-Path $repoRoot 'package.json'))) {
    Write-Err "package.json not found at $repoRoot"
    Write-Err "This script must live inside the Handy repository under scripts\."
    exit 1
}

# ── Build ─────────────────────────────────────────────────────────────────────

Write-Step "Building release binary (bun run tauri build --no-bundle)"
Write-Info "Repo root: $repoRoot"
Write-Info "This may take several minutes on the first run."

$ErrorActionPreference = 'Stop'
try {
    Push-Location $repoRoot
    bun run tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed with exit code $LASTEXITCODE"
    }
    Write-Success "Build completed"
} catch {
    Write-Err "Build failed: $_"
    Write-Info "Tip: Make sure VULKAN_SDK is set (open a fresh terminal after installing the Vulkan SDK)."
    Write-Info "Tip: If you see MAX_PATH errors, run windows-setup.ps1 to set CARGO_TARGET_DIR=C:\h"
    exit 1
} finally {
    Pop-Location
    $ErrorActionPreference = 'Continue'
}

# ── Verify the binary was produced ───────────────────────────────────────────

if (-not (Test-Path $builtExe)) {
    Write-Err "Expected binary not found at: $builtExe"
    Write-Info "The build may have succeeded but written output elsewhere."
    Write-Info "Check the build log above for 'Built application at:' to find the actual path."
    exit 1
}

Write-Success "Binary found: $builtExe"

# ── Kill running instance ─────────────────────────────────────────────────────

Write-Step "Stopping any running handy.exe"

# SilentlyContinue is intentional — not an error if handy isn't running.
$proc = Get-Process -Name handy -ErrorAction SilentlyContinue
if ($proc) {
    Stop-Process -Name handy -Force -ErrorAction SilentlyContinue
    # Give the OS a moment to release file locks
    Start-Sleep -Milliseconds 800
    Write-Success "Stopped running handy.exe (PID $($proc.Id))"
} else {
    Write-Info "No running handy.exe found — nothing to stop"
}

# ── Create install directory ──────────────────────────────────────────────────

Write-Step "Creating install directory"

$ErrorActionPreference = 'Stop'
try {
    if (-not (Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        Write-Success "Created: $installDir"
    } else {
        Write-Info "Directory already exists: $installDir"
    }
} catch {
    Write-Err "Could not create install directory $installDir : $_"
    exit 1
} finally {
    $ErrorActionPreference = 'Continue'
}

# ── Copy binary ───────────────────────────────────────────────────────────────

Write-Step "Installing handy.exe"

$ErrorActionPreference = 'Stop'
try {
    Copy-Item -Path $builtExe -Destination $installExe -Force
    Write-Success "Installed to: $installExe"
} catch {
    Write-Err "Failed to copy binary: $_"
    Write-Info "Make sure handy.exe is not still running and try again."
    exit 1
} finally {
    $ErrorActionPreference = 'Continue'
}

# ── Launch ────────────────────────────────────────────────────────────────────

Write-Step "Launching Handy"

try {
    Start-Process -FilePath $installExe
    Write-Success "Handy launched"
} catch {
    Write-Warning-Custom "Failed to launch: $_"
    Write-Info "You can start it manually: $installExe"
}

# ── Summary ───────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host "  Dev install complete!" -ForegroundColor Green
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host ""
Write-Host "  Installed: $installExe" -ForegroundColor White
Write-Host ""
Write-Host "  Notes:" -ForegroundColor White
Write-Host "  · Models persist across rebuilds in:" -ForegroundColor Gray
Write-Host "      $env:APPDATA\com.pais.handy\models\" -ForegroundColor Gray
Write-Host "  · For a hot-reload dev session (no install needed), use:" -ForegroundColor Gray
Write-Host "      bun run tauri dev" -ForegroundColor Yellow
Write-Host ""
