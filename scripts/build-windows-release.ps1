#Requires -Version 5.1
<#
.SYNOPSIS
  Build a Windows 10+ release installer, Authenticode-sign it, GnuPG-sign it,
  and publish to ros-website/public/downloads/.

.NOTES
  Run on a Windows 10/11 machine (or Win11 VM). Do not run from Linux.
  Product compatibility target: Windows 10 and later.
  Requires reviewed production SQLCipher artifacts for x86_64-pc-windows-msvc.
#>
param()

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$AppDir = Join-Path $Root "apps\ros"
$OutDir = Join-Path $Root "ros-website\public\downloads"
$SigningRoot = Join-Path $Root "secrets\signing"
$PfxPath = Join-Path $SigningRoot "windows\ros-codesign.pfx"
$PfxPasswordPath = Join-Path $SigningRoot "windows\ros-codesign.password"
$GpgSec = Join-Path $SigningRoot "gpg\gotigin-ros-release.sec"
$IssTemplate = Join-Path $PSScriptRoot "windows\ros-windows-installer.iss"
$BinaryName = "ros-windows-x64.exe"
$SigName = "$BinaryName.sig"
$GpgHome = $null

# Flutter Windows plugin_symlinks need real NTFS symlinks. Guest shared folders
# (VirtualBox/VMware Z:) usually deny that with ERROR_ACCESS_DENIED.
try {
  $RootDrive = [System.IO.DriveInfo]::new((Split-Path -Qualifier $Root))
  if ($RootDrive.DriveType -ne 'Fixed') {
    throw @"
Refusing to build on a non-fixed drive ($Root).

Flutter cannot create plugin symlinks on VirtualBox/VMware shared folders.
Copy the tree to a local NTFS path and build there, for example:

  robocopy Z:\WindowsShared\output C:\ros-build /E /XD .git build .dart_tool
  cd C:\ros-build
  .\scripts\build-windows-release.ps1

Then copy ros-website\public\downloads\ros-windows-x64.exe* back to the share.
Also enable Windows Developer Mode (Settings -> System -> For developers) so
symlink creation works without elevation on the local disk.
"@
  }
} catch [System.Management.Automation.RuntimeException] {
  throw
} catch {
  # DriveInfo failed; continue and let Flutter report symlink errors.
}

function Remove-GpgHome {
  if ($null -ne $GpgHome -and (Test-Path $GpgHome)) {
    Remove-Item -Recurse -Force $GpgHome -ErrorAction SilentlyContinue
    $script:GpgHome = $null
    Remove-Item Env:GNUPGHOME -ErrorAction SilentlyContinue
  }
}

trap {
  Remove-GpgHome
  break
}

function Test-ProductionSqlcipherWindows {
  $Manifest = Join-Path $Root "third_party\sqlcipher\MANIFEST.toml"
  if (-not (Test-Path $Manifest)) { return $false }
  $Text = Get-Content -Raw $Manifest
  if ($Text -match 'reviewed_by = ""') { return $false }
  if ($Text -notmatch 'target = "x86_64-pc-windows-msvc"') { return $false }
  return $true
}

function Export-ProductionSqlcipherEnv {
  $Manifest = Join-Path $Root "third_party\sqlcipher\MANIFEST.toml"
  $Lines = Get-Content $Manifest
  $Target = $null
  $Path = $null
  foreach ($Line in $Lines) {
    $Trim = $Line.Trim()
    if ($Trim -eq "[[artifact]]") {
      $Target = $null
      $Path = $null
      continue
    }
    if ($Trim -match '^target\s*=\s*"([^"]+)"') { $Target = $Matches[1] }
    if ($Trim -match '^path\s*=\s*"([^"]+)"') { $Path = $Matches[1] }
    if ($Target -eq "x86_64-pc-windows-msvc" -and $Path) {
      $SearchDir = Split-Path (Join-Path $Root "third_party\sqlcipher\$Path") -Parent
      $env:SQLCIPHER_LIB_DIR = $SearchDir
      $env:SQLCIPHER_STATIC = "1"
      $env:SQLITE3_LIB_DIR = $SearchDir
      $IncludeDir = Join-Path $SearchDir "include"
      if (Test-Path $IncludeDir) {
        $env:SQLCIPHER_INCLUDE_DIR = $IncludeDir
        $env:SQLITE3_INCLUDE_DIR = $IncludeDir
      }
      Write-Host "SQLCIPHER_LIB_DIR=$($env:SQLCIPHER_LIB_DIR)"
      return
    }
  }
  throw "Could not resolve windows SQLCipher artifact path in MANIFEST.toml"
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if (-not (Test-Path $PfxPath) -or -not (Test-Path $PfxPasswordPath)) {
  throw "Missing Windows Authenticode material. Run .\scripts\generate-release-signing-keys.ps1 first."
}
if (-not (Test-Path $GpgSec)) {
  throw "Missing GnuPG release secret at $GpgSec. Generate keys on Linux or Windows first."
}
if (-not (Get-Command flutter -ErrorAction SilentlyContinue)) {
  throw "flutter not found on PATH."
}
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  throw @"
rustup not found on PATH (required by Cargokit / Flutter Rust bridge).

Install Rust for Windows from https://rustup.rs/ (default MSVC toolchain),
then open a NEW PowerShell and confirm:

  rustup --version
  rustc --version

cargokit.yaml pins toolchain 1.97.0; rustup will download it on first build:
  rustup toolchain install 1.97.0
"@
}
if (-not (Get-Command gpg -ErrorAction SilentlyContinue)) {
  throw "gpg not found on PATH (install Gpg4win)."
}

$SignTool = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue |
  Sort-Object FullName -Descending |
  Select-Object -First 1
if ($null -eq $SignTool) {
  throw "signtool.exe not found. Install Windows 10/11 SDK signing tools."
}

$Iscc = @(
  "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
  "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $Iscc) {
  throw "Inno Setup 6 (ISCC.exe) not found. Install from https://jrsoftware.org/isinfo.php"
}

if (-not (Test-ProductionSqlcipherWindows)) {
  throw @"
Production SQLCipher artifacts for x86_64-pc-windows-msvc are not ready.
Place reviewed libraries under third_party/sqlcipher/ per
docs/runbooks/sqlcipher-artifact-manifest.md.
Do not use the development SQLCipher feature for Release packaging.
"@
}
Export-ProductionSqlcipherEnv

# production-sqlcipher enables rusqlite/buildtime_bindgen — needs libclang.
if (-not $env:LIBCLANG_PATH) {
  $LlvmCandidates = @(
    "${env:ProgramFiles}\LLVM\bin",
    "${env:ProgramFiles}\Microsoft Visual Studio\18\Community\VC\Tools\Llvm\x64\bin",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\VC\Tools\Llvm\x64\bin",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional\VC\Tools\Llvm\x64\bin",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools\VC\Tools\Llvm\x64\bin"
  )
  foreach ($Dir in $LlvmCandidates) {
    if (Test-Path (Join-Path $Dir "libclang.dll")) {
      $env:LIBCLANG_PATH = $Dir
      break
    }
  }
}
if (-not $env:LIBCLANG_PATH) {
  throw @"
LIBCLANG_PATH is not set and libclang.dll was not found.

Install LLVM (https://github.com/llvm/llvm-project/releases) or the VS
"C++ Clang Compiler for Windows" component, then either re-run or set:
  `$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
"@
}
Write-Host "LIBCLANG_PATH=$($env:LIBCLANG_PATH)"

# Static sqlcipher.lib needs OpenSSL import libs on the MSVC link line.
if (-not $env:OPENSSL_ROOT_DIR) {
  foreach ($Candidate in @(
      "C:\Program Files\OpenSSL-Win64",
      "C:\Program Files\OpenSSL"
    )) {
    if (Test-Path (Join-Path $Candidate "include\openssl\ssl.h")) {
      $env:OPENSSL_ROOT_DIR = $Candidate
      break
    }
  }
}
if ($env:OPENSSL_ROOT_DIR) {
  Write-Host "OPENSSL_ROOT_DIR=$($env:OPENSSL_ROOT_DIR)"
  foreach ($LibDir in @(
      (Join-Path $env:OPENSSL_ROOT_DIR "lib\VC\x64\MD"),
      (Join-Path $env:OPENSSL_ROOT_DIR "lib\VC\x64\MT"),
      (Join-Path $env:OPENSSL_ROOT_DIR "lib")
    )) {
    if (Test-Path (Join-Path $LibDir "libcrypto.lib")) {
      $env:OPENSSL_LIB_DIR = $LibDir
      $env:LIB = "$LibDir;$($env:LIB)"
      Write-Host "OPENSSL_LIB_DIR=$LibDir"
      break
    }
  }
}

$env:CARGOKIT_VERBOSE = "1"

Push-Location $AppDir
try {
  flutter pub get --enforce-lockfile
  flutter build windows --release -v
  if ($LASTEXITCODE -ne 0) {
    throw @"
flutter build windows --release failed (exit $LASTEXITCODE).

Scroll up for the cargo/cargokit error (often libclang / SQLCipher / OpenSSL).
Re-run with the same env after fixing, or paste the first rustc/cargo error block.
"@
  }
}
finally {
  Pop-Location
}

$ReleaseDir = Join-Path $AppDir "build\windows\x64\runner\Release"
$ExePath = Join-Path $ReleaseDir "ros.exe"
if (-not (Test-Path $ExePath)) {
  throw "Expected Windows release binary at $ExePath"
}

# Authenticode-sign the application binary and companion DLLs before packaging.
$PfxPassword = Get-Content -Raw $PfxPasswordPath
Get-ChildItem -Path $ReleaseDir -Include *.exe,*.dll -Recurse | ForEach-Object {
  & $SignTool.FullName sign `
    /f $PfxPath `
    /p $PfxPassword `
    /fd SHA256 `
    /td SHA256 `
    /tr http://timestamp.digicert.com `
    $_.FullName
  if ($LASTEXITCODE -ne 0) {
    Write-Warning "Timestamping failed for $($_.Name); retrying without timestamp server."
    & $SignTool.FullName sign `
      /f $PfxPath `
      /p $PfxPassword `
      /fd SHA256 `
      $_.FullName
    if ($LASTEXITCODE -ne 0) {
      throw "Authenticode signing failed for $($_.FullName)"
    }
  }
}

$IssWork = Join-Path $env:TEMP ("ros-iss-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $IssWork | Out-Null
$IssPath = Join-Path $IssWork "ros-windows-installer.iss"
$ReleaseDirUnix = $ReleaseDir -replace '\\', '/'
$OutDirUnix = $OutDir -replace '\\', '/'
$IssContent = Get-Content -Raw $IssTemplate
$IssContent = $IssContent.Replace("{{SOURCE_DIR}}", $ReleaseDirUnix)
$IssContent = $IssContent.Replace("{{OUTPUT_DIR}}", $OutDirUnix)
$IssContent = $IssContent.Replace("{{OUTPUT_BASENAME}}", [System.IO.Path]::GetFileNameWithoutExtension($BinaryName))
Set-Content -Path $IssPath -Value $IssContent -Encoding ASCII

& $Iscc $IssPath
if ($LASTEXITCODE -ne 0) {
  throw "Inno Setup compilation failed."
}

$InstallerPath = Join-Path $OutDir $BinaryName
if (-not (Test-Path $InstallerPath)) {
  throw "Expected installer at $InstallerPath"
}

& $SignTool.FullName sign `
  /f $PfxPath `
  /p $PfxPassword `
  /fd SHA256 `
  /td SHA256 `
  /tr http://timestamp.digicert.com `
  $InstallerPath
if ($LASTEXITCODE -ne 0) {
  Write-Warning "Installer timestamping failed; signing without timestamp."
  & $SignTool.FullName sign /f $PfxPath /p $PfxPassword /fd SHA256 $InstallerPath
  if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed for installer." }
}

$GpgHome = Join-Path $env:TEMP ("ros-gpg-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $GpgHome | Out-Null
$env:GNUPGHOME = $GpgHome
# gpg writes progress to stderr; with $ErrorActionPreference=Stop that becomes a
# terminating NativeCommandError even when exit code is 0.
$PrevEap = $ErrorActionPreference
$ErrorActionPreference = "Continue"
try {
  & gpg --batch --import $GpgSec 2>&1 | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "GnuPG import of release secret failed (exit $LASTEXITCODE)." }
  $SigPath = Join-Path $OutDir $SigName
  if (Test-Path $SigPath) { Remove-Item $SigPath -Force }
  & gpg --batch --yes --detach-sign --armor -o $SigPath $InstallerPath 2>&1 | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "GnuPG detached signature failed (exit $LASTEXITCODE)." }
} finally {
  $ErrorActionPreference = $PrevEap
}

Remove-GpgHome
Remove-Item -Recurse -Force $IssWork -ErrorAction SilentlyContinue

$Hash = (Get-FileHash -Algorithm SHA256 $InstallerPath).Hash.ToLowerInvariant()
Write-Host ""
Write-Host "Published:"
Write-Host "  $InstallerPath"
Write-Host "  $SigPath"
Write-Host "  sha256=$Hash"
Write-Host "Minimum OS: Windows 10 (x64) and later."
