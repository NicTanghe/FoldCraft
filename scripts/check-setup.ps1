param([switch]$RequireGhAuth)

# Read-only checks; this script does not install tools or build the project.
$ErrorActionPreference = 'Continue'
$missing = 0
function Check-Command($Label, $Name, $Arguments, $Remedy, $QuietFailure = $false) {
    $command = Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) {
        $output = & $command.Source @Arguments 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "[OK] $Label"
            return $true
        }
    }
    if (-not $QuietFailure) { Write-Host "[MISSING] $Label. $Remedy" }
    return $false
}
function Require-Check($Passed) { if (-not $Passed) { $script:missing++ } }

Write-Host 'ClothCraft Windows setup check (x64 MSVC)'
Require-Check (Check-Command 'Git' 'git.exe' @('--version') 'Install Git for Windows and reopen PowerShell.')
$python = (Check-Command 'Python 3' 'python.exe' @('-c', 'import sys; sys.exit(0 if sys.version_info.major == 3 else 1)') 'Install Python 3 or enable its PATH entry.' $true)
if (-not $python) {
    $python = Check-Command 'Python 3 launcher' 'py.exe' @('-3', '-c', 'import sys') 'Install Python 3 and reopen PowerShell.'
}
Require-Check $python
Require-Check (Check-Command 'Rust compiler' 'rustc.exe' @('--version') 'Install Rust with rustup and select an MSVC toolchain.')
Require-Check (Check-Command 'Cargo' 'cargo.exe' @('--version') 'Install Rust with rustup and reopen PowerShell.')
Require-Check (Check-Command 'rustfmt' 'cargo.exe' @('fmt', '--version') 'Run: rustup component add rustfmt')
Require-Check (Check-Command 'Clippy' 'cargo.exe' @('clippy', '--version') 'Run: rustup component add clippy')
$rustc = Get-Command rustc.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
$hostLine = if ($rustc) { & $rustc.Source -vV 2>$null | Where-Object { $_ -like 'host: *' } } else { '' }
if ($hostLine -eq 'host: x86_64-pc-windows-msvc') {
    Write-Host '[OK] Rust host: x86_64-pc-windows-msvc'
} else {
    Write-Host '[MISSING] x64 MSVC Rust host. Install/select x86_64-pc-windows-msvc with rustup.'
    $missing++
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$linker = $null
if (Test-Path -LiteralPath $vswhere) {
    $installations = & $vswhere -all -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    foreach ($installation in $installations) {
        $linker = Get-ChildItem -Path (Join-Path $installation 'VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe') -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($linker) { break }
    }
}
if ($linker) { Write-Host '[OK] Visual C++ x64 linker' } else {
    Write-Host '[MISSING] Visual C++ x64 linker. Install Visual Studio Build Tools with Desktop development with C++ (MSVC x64/x86 build tools).'
    $missing++
}
$kits = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots' -Name KitsRoot10 -ErrorAction SilentlyContinue).KitsRoot10
$sdk = $false
if ($kits) {
    foreach ($include in (Get-ChildItem -Path (Join-Path $kits 'Include\*') -Directory -ErrorAction SilentlyContinue)) {
        if ((Test-Path -LiteralPath (Join-Path $include.FullName 'um\Windows.h')) -and
            (Test-Path -LiteralPath (Join-Path $kits "Lib\$($include.Name)\um\x64\kernel32.lib"))) { $sdk = $true; break }
    }
}
if ($sdk) { Write-Host '[OK] Windows SDK x64 headers and libraries' } else {
    Write-Host '[MISSING] Windows SDK x64 headers and libraries. Add a Windows 10 or 11 SDK in Visual Studio Installer.'
    $missing++
}

$gh = Get-Command gh.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if ($gh) {
    & $gh.Source auth status *> $null
    if ($LASTEXITCODE -eq 0) { Write-Host '[OK] GitHub CLI authentication' } else {
        if ($RequireGhAuth) { $missing++; $tag = 'MISSING' } else { $tag = 'OPTIONAL' }
        Write-Host "[$tag] GitHub CLI is not authenticated. For PR publishing, run: gh auth login"
    }
} else {
    if ($RequireGhAuth) { $missing++; $tag = 'MISSING' } else { $tag = 'OPTIONAL' }
    Write-Host "[$tag] GitHub CLI not found. For PR publishing, install gh and run: gh auth login"
}
if ($missing -gt 0) { Write-Host "Setup incomplete: $missing required check(s) failed."; exit 1 }
Write-Host 'Setup checks passed. Run .\scripts\check-cpu.ps1 to validate the project.'
exit 0
