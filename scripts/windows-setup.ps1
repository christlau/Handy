#Requires -Version 5.1
<#
.SYNOPSIS
    Handy Windows development environment setup script.

.DESCRIPTION
    Installs all prerequisites for building and running Handy on Windows:
    - Microsoft Visual C++ Build Tools 2022
    - CMake
    - Vulkan SDK
    - Rust (via rustup)
    - Bun package manager
    - Node.js LTS (fallback)

    Also sets CARGO_TARGET_DIR to a short path (C:\h) to avoid Windows'
    260-character MAX_PATH limit during native Vulkan/CMake builds.

.NOTES
    Run this once when setting up a new Windows machine.
    After running, open a new terminal and use: bun run tauri dev
#>

# ── Helpers ─────────────────────────────────────────────────────────────────

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

# ── Admin check ──────────────────────────────────────────────────────────────

Write-Step "Checking privileges"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)

if ($isAdmin) {
    Write-Success "Running as Administrator"
} else {
    Write-Warning-Custom "Not running as Administrator."
    Write-Info "Most installs work without admin rights, but Visual C++ Build Tools"
    Write-Info "may require elevation. If a winget install fails, re-run this script"
    Write-Info "from an elevated PowerShell prompt."
}

# ── winget availability check ────────────────────────────────────────────────

Write-Step "Checking winget availability"

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Err "winget not found. Install 'App Installer' from the Microsoft Store,"
    Write-Err "or download it from: https://aka.ms/getwinget"
    exit 1
}

Write-Success "winget is available"

# ── Install helper ───────────────────────────────────────────────────────────

function Install-IfMissing {
    <#
    .SYNOPSIS
        Installs a package via winget only if it is not already present.
    .PARAMETER WingetId
        The winget package ID (e.g. "Kitware.CMake").
    .PARAMETER DisplayName
        Human-readable name shown in output.
    .PARAMETER ExtraArgs
        Additional arguments forwarded to winget install (array).
    #>
    param(
        [string]$WingetId,
        [string]$DisplayName,
        [string[]]$ExtraArgs = @()
    )

    Write-Step "Checking $DisplayName ($WingetId)"

    # winget list returns exit code 0 even when not found; grep the output instead.
    # Use --accept-source-agreements to suppress interactive prompts.
    $listOutput = winget list --id $WingetId --accept-source-agreements 2>&1
    $alreadyInstalled = ($listOutput | Select-String -Pattern ([regex]::Escape($WingetId))) -ne $null

    if ($alreadyInstalled) {
        Write-Success "$DisplayName is already installed — skipping"
        return
    }

    Write-Info "Installing $DisplayName ..."

    # winget returns non-zero for certain success conditions (e.g. already installed
    # race, reboot required).  We intentionally do NOT use $ErrorActionPreference =
    # 'Stop' here so the script continues even if winget signals a soft failure.
    $installArgs = @('install', '--id', $WingetId, '--accept-package-agreements',
                     '--accept-source-agreements', '--disable-interactivity') + $ExtraArgs
    winget @installArgs

    if ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq 3010) {
        # 3010 = success, reboot required
        Write-Success "$DisplayName installed successfully"
        if ($LASTEXITCODE -eq 3010) {
            Write-Warning-Custom "A system reboot is recommended to complete the $DisplayName installation."
        }
    } else {
        Write-Warning-Custom "$DisplayName install exited with code $LASTEXITCODE."
        Write-Info "This may be harmless (e.g. already installed via another mechanism)."
        Write-Info "Continue and check manually if you encounter build errors."
    }
}

# ── Install prerequisites ────────────────────────────────────────────────────

# Visual C++ Build Tools 2022 — needs --override to select workload
Install-IfMissing `
    -WingetId    'Microsoft.VisualStudio.2022.BuildTools' `
    -DisplayName 'Visual C++ Build Tools 2022' `
    -ExtraArgs   @('--override', '--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended')

Install-IfMissing `
    -WingetId    'Kitware.CMake' `
    -DisplayName 'CMake'

Install-IfMissing `
    -WingetId    'KhronosGroup.VulkanSDK' `
    -DisplayName 'Vulkan SDK'

Install-IfMissing `
    -WingetId    'Rustlang.Rustup' `
    -DisplayName 'Rust (rustup)'

Install-IfMissing `
    -WingetId    'Oven-sh.Bun' `
    -DisplayName 'Bun'

Install-IfMissing `
    -WingetId    'OpenJS.NodeJS.LTS' `
    -DisplayName 'Node.js LTS'

# ── CARGO_TARGET_DIR (short path to avoid MAX_PATH issues) ───────────────────

Write-Step "Setting CARGO_TARGET_DIR to C:\h"

$ErrorActionPreference = 'Stop'
try {
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', 'C:\h', 'User')
    $env:CARGO_TARGET_DIR = 'C:\h'
    Write-Success "CARGO_TARGET_DIR set to C:\h (user environment, persisted)"
    Write-Info "Tauri build artifacts will land in C:\h\release\ instead of"
    Write-Info "the repo's src-tauri\target\ — this avoids MAX_PATH overflows"
    Write-Info "during the Vulkan shader generator's nested CMake build."
} catch {
    Write-Err "Failed to set CARGO_TARGET_DIR: $_"
    Write-Info "Set it manually: [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', 'C:\h', 'User')"
}
$ErrorActionPreference = 'Continue'

# ── Refresh PATH in current session ─────────────────────────────────────────

Write-Step "Refreshing PATH from registry"

try {
    $machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $userPath    = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:PATH    = ($machinePath, $userPath | Where-Object { $_ }) -join ';'
    Write-Success "PATH refreshed — newly installed tools should be available now"
} catch {
    Write-Warning-Custom "Could not refresh PATH automatically: $_"
    Write-Info "Open a new terminal to pick up the updated PATH."
}

# ── Install bun dependencies ─────────────────────────────────────────────────

Write-Step "Installing bun dependencies (bun install)"

# Resolve repo root relative to this script's location
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not (Test-Path (Join-Path $repoRoot 'package.json'))) {
    Write-Err "package.json not found at $repoRoot"
    Write-Info "Make sure you are running this script from inside the Handy repo."
    exit 1
}

$ErrorActionPreference = 'Stop'
try {
    Push-Location $repoRoot
    Write-Info "Working directory: $repoRoot"

    if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
        Write-Warning-Custom "bun not found in PATH yet."
        Write-Info "This is expected if Bun was just installed in this same session."
        Write-Info "Open a new terminal and run:  bun install"
    } else {
        bun install
        if ($LASTEXITCODE -ne 0) {
            throw "bun install failed with exit code $LASTEXITCODE"
        }
        Write-Success "bun install completed"
    }
} catch {
    Write-Err "bun install failed: $_"
    Write-Info "Open a new terminal (to pick up updated PATH) and run: bun install"
} finally {
    Pop-Location
    $ErrorActionPreference = 'Continue'
}

# ── Success summary ──────────────────────────────────────────────────────────

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host "  Setup complete!  Open a NEW terminal so VULKAN_SDK and PATH are set." -ForegroundColor Green
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host ""
Write-Host "  Next steps:" -ForegroundColor White
Write-Host ""
Write-Host "  1. Open a new PowerShell / Windows Terminal window" -ForegroundColor White
Write-Host "  2. Navigate to the repo root:" -ForegroundColor White
Write-Host "       cd $repoRoot" -ForegroundColor Yellow
Write-Host "  3. If you skipped bun install above, run it now:" -ForegroundColor White
Write-Host "       bun install" -ForegroundColor Yellow
Write-Host "  4. Start the dev loop:" -ForegroundColor White
Write-Host "       bun run tauri dev" -ForegroundColor Yellow
Write-Host ""
Write-Host "  Notes:" -ForegroundColor White
Write-Host "  · Models are stored in and persist across rebuilds:" -ForegroundColor Gray
Write-Host "      $env:APPDATA\com.pais.handy\models\" -ForegroundColor Gray
Write-Host "  · Build artifacts land in C:\h\release\ (short path, avoids MAX_PATH)" -ForegroundColor Gray
Write-Host "  · To build and install locally without bundling/signing:" -ForegroundColor Gray
Write-Host "      bun run tauri build --no-bundle" -ForegroundColor Yellow
Write-Host "    then copy C:\h\release\handy.exe to your preferred location," -ForegroundColor Gray
Write-Host "    or run scripts\windows-install-dev.ps1 to do it automatically." -ForegroundColor Gray
Write-Host ""
