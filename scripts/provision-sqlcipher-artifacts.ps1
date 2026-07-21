#Requires -Version 5.1
<#
.SYNOPSIS
  Provision pinned SQLCipher 4.17.0 static library for Windows x64 (MSVC).

.DESCRIPTION
  Downloads the official SQLCipher v4.17.0 source tarball, builds sqlcipher.lib
  with OpenSSL, installs under third_party/sqlcipher/windows-x86_64/, and merges
  the Windows [[artifact]] into MANIFEST.toml (preserving the Linux entry when
  present).

  Run on a Windows 10/11 build host with Visual Studio C++ and OpenSSL.
#>
param(
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$Version = "4.17.0"
$TarballUrl = "https://github.com/sqlcipher/sqlcipher/archive/refs/tags/v$Version.tar.gz"
$TarballSha256 = "79c0e164b9c059e7487bf8f29272f601cca5f3312cc267461f81e349962a5058"
$ProvenanceUrl = "https://github.com/sqlcipher/sqlcipher/releases/tag/v$Version"

$OutRoot = Join-Path $Root "third_party\sqlcipher"
$ArtifactDir = Join-Path $OutRoot "windows-x86_64"
$ArtifactLib = Join-Path $ArtifactDir "sqlcipher.lib"
$IncludeDir = Join-Path $ArtifactDir "include"

# Download + compile on a local disk. Building under a VirtualBox/VMware shared
# folder (Z:) is extremely slow and looks "stuck" with little output.
$UseLocalBuildCache = $true
try {
  $Drive = [System.IO.DriveInfo]::new((Split-Path -Qualifier $Root))
  # Fixed local disks can keep cache in-repo; Network/Unknown (guest shares) use TEMP.
  $UseLocalBuildCache = $Drive.DriveType -ne 'Fixed'
} catch {
  $UseLocalBuildCache = $true
}
if ($env:ROS_SQLCIPHER_BUILD_DIR) {
  $CacheDir = $env:ROS_SQLCIPHER_BUILD_DIR
} elseif ($UseLocalBuildCache) {
  $CacheDir = Join-Path $env:TEMP "ros-sqlcipher-src"
  Write-Host "Repo path looks shared/non-local ($Root). Using local build cache: $CacheDir"
} else {
  $CacheDir = Join-Path $Root "tools\sqlcipher-src"
}
$Tarball = Join-Path $CacheDir "sqlcipher-$Version.tar.gz"
$SrcDir = Join-Path $CacheDir "sqlcipher-$Version"

New-Item -ItemType Directory -Force -Path $CacheDir, $ArtifactDir, $IncludeDir | Out-Null

function Get-FileSha256Hex([string]$Path) {
  return (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLowerInvariant()
}

if ((Test-Path $ArtifactLib) -and -not $Force) {
  Write-Host "Windows artifact already present: $ArtifactLib"
  Write-Host "Pass -Force to rebuild."
} else {
  if (-not (Test-Path $Tarball)) {
    Write-Host "Downloading SQLCipher $Version (may take a few minutes)..."
    $Partial = "$Tarball.partial"
    $ProgressPreference = 'Continue'
    try {
      # Prefer curl.exe when present (better progress than older Invoke-WebRequest).
      $Curl = Get-Command curl.exe -ErrorAction SilentlyContinue
      if ($Curl) {
        & curl.exe -L --fail --show-error --progress-bar -o $Partial $TarballUrl
        if ($LASTEXITCODE -ne 0) { throw "curl download failed (exit $LASTEXITCODE)" }
      } else {
        Invoke-WebRequest -Uri $TarballUrl -OutFile $Partial
      }
      Move-Item -Force $Partial $Tarball
    } catch {
      Remove-Item -Force $Partial -ErrorAction SilentlyContinue
      throw
    }
    Write-Host "Download complete."
  } else {
    Write-Host "Using cached tarball: $Tarball"
  }

  Write-Host "Verifying tarball checksum..."
  $Actual = Get-FileSha256Hex $Tarball
  if ($Actual -ne $TarballSha256) {
    throw "Tarball checksum mismatch. expected=$TarballSha256 actual=$Actual"
  }

  Write-Host "Extracting source to $SrcDir ..."
  if (Test-Path $SrcDir) { Remove-Item -Recurse -Force $SrcDir }
  tar -xzf $Tarball -C $CacheDir
  if (-not (Test-Path $SrcDir)) {
    throw "Expected extracted source at $SrcDir"
  }
  Write-Host "Extract complete."

  # Prefer vswhere (handles Community / Pro / Enterprise / Build Tools / custom paths).
  # Hardcoded paths miss Build Tools and non-default install locations.
  $VsDevCmd = $null
  $VsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $VsWhere) {
    $InstallPath = & $VsWhere -latest -products * `
      -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
      -property installationPath 2>$null |
      Select-Object -First 1
    if (-not $InstallPath) {
      $InstallPath = & $VsWhere -latest -products * -property installationPath 2>$null |
        Select-Object -First 1
    }
    if ($InstallPath) {
      $Candidate = Join-Path $InstallPath.Trim() "Common7\Tools\VsDevCmd.bat"
      if (Test-Path $Candidate) { $VsDevCmd = $Candidate }
    }
  }
  if (-not $VsDevCmd) {
    $VsDevCmd = @(
      "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat",
      "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat",
      "${env:ProgramFiles}\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat",
      "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat",
      "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\Community\Common7\Tools\VsDevCmd.bat",
      "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\BuildTools\Common7\Tools\VsDevCmd.bat"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
  }
  if (-not $VsDevCmd) {
    throw @'
VsDevCmd.bat not found. This is unrelated to the Z: shared folder (VS installs on the Windows disk).

Install or repair Visual Studio with the "Desktop development with C++" workload,
or "Build Tools" with the MSVC x64/x86 toolset. Then either:
  1) Re-run this script, or
  2) Open "Developer PowerShell for VS 2022" from the Start menu and run it from there.

Quick check:
  & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath
'@
  }
  Write-Host "Using VsDevCmd: $VsDevCmd"

  # Prefer OPENSSL_ROOT_DIR or vcpkg classic layout.
  $OpenSslRoot = $env:OPENSSL_ROOT_DIR
  if (-not $OpenSslRoot) {
    $Candidates = @(
      "C:\Program Files\OpenSSL-Win64",
      "C:\vcpkg\installed\x64-windows",
      "$env:VCPKG_ROOT\installed\x64-windows"
    ) | Where-Object { $_ -and (Test-Path $_) }
    $OpenSslRoot = $Candidates | Select-Object -First 1
  }
  if (-not $OpenSslRoot) {
    throw "Set OPENSSL_ROOT_DIR to an OpenSSL install (include/ + lib/)."
  }

  Write-Host "Using OpenSSL root: $OpenSslRoot"

  $OpenSslInclude = $null
  foreach ($Inc in @(
      (Join-Path $OpenSslRoot "include"),
      (Join-Path $OpenSslRoot "include\openssl"),
      (Join-Path $OpenSslRoot "..\include")
    )) {
    $Probe = if ((Split-Path -Leaf $Inc) -eq "openssl") {
      Join-Path (Split-Path $Inc -Parent) "openssl\ssl.h"
    } else {
      Join-Path $Inc "openssl\ssl.h"
    }
    $IncRoot = if ((Split-Path -Leaf $Inc) -eq "openssl") { Split-Path $Inc -Parent } else { $Inc }
    if (Test-Path (Join-Path $IncRoot "openssl\ssl.h")) {
      $OpenSslInclude = $IncRoot
      break
    }
  }
  if (-not $OpenSslInclude) {
    throw @"
OpenSSL headers (include\openssl\ssl.h) were not found under:
  $OpenSslRoot

Install the FULL Win64 OpenSSL package (not "Light") from
https://slproweb.com/products/Win32OpenSSL.html
so that this exists:
  C:\Program Files\OpenSSL-Win64\include\openssl\ssl.h

Then re-run, or set OPENSSL_ROOT_DIR to that install root.
"@
  }
  Write-Host "Using OpenSSL include: $OpenSslInclude"

  $OpenSslLibDirs = @(
    (Join-Path $OpenSslRoot "lib\VC\x64\MD"),
    (Join-Path $OpenSslRoot "lib\VC\x64\MT"),
    (Join-Path $OpenSslRoot "lib\VC\static"),
    (Join-Path $OpenSslRoot "lib")
  ) | Where-Object { Test-Path $_ }

  $CryptoLib = $null
  $SslLib = $null
  $OpenSslLibDir = $null
  foreach ($LibDir in $OpenSslLibDirs) {
    foreach ($Pair in @(
        @("libcrypto.lib", "libssl.lib"),
        @("libcrypto64MD.lib", "libssl64MD.lib"),
        @("libcrypto64MT.lib", "libssl64MT.lib")
      )) {
      if ((Test-Path (Join-Path $LibDir $Pair[0])) -and (Test-Path (Join-Path $LibDir $Pair[1]))) {
        $OpenSslLibDir = $LibDir
        $CryptoLib = $Pair[0]
        $SslLib = $Pair[1]
        break
      }
    }
    if ($OpenSslLibDir) { break }
  }
  if (-not $OpenSslLibDir) {
    throw "Could not find OpenSSL import libs under $OpenSslRoot (tried lib/ and lib/VC/...)."
  }
  Write-Host "Using OpenSSL libs: $OpenSslLibDir ($CryptoLib, $SslLib)"
  Write-Host "Compiling SQLCipher with nmake (often 5-15 minutes; watch for cl.exe CPU)..."

  # nmake/cl mangle unquoted paths with spaces (C:\Program Files -> /IC:\Program).
  # Prefer 8.3 short paths; also prepend INCLUDE/LIB so /I and /LIBPATH are unnecessary.
  function Get-ShortPath([string]$Path) {
    try {
      $fso = New-Object -ComObject Scripting.FileSystemObject
      if (Test-Path -Path $Path -PathType Container) {
        return $fso.GetFolder($Path).ShortPath
      }
      return $fso.GetFile($Path).ShortPath
    } catch {
      return $Path
    }
  }
  $OpenSslIncludeShort = Get-ShortPath $OpenSslInclude
  $OpenSslLibDirShort = Get-ShortPath $OpenSslLibDir
  Write-Host "Using short include path: $OpenSslIncludeShort"
  Write-Host "Using short lib path: $OpenSslLibDirShort"

  $BuildBat = Join-Path $env:TEMP ("ros-sqlcipher-build-" + [guid]::NewGuid().ToString("N") + ".bat")
  # nmake clean removes sqlite3.c; regenerate amalgamation via jimsh0 before linking.
  $Opts = "-DSQLITE_HAS_CODEC -DSQLCIPHER_CRYPTO_OPENSSL -DSQLITE_TEMP_STORE=2 -DSQLITE_EXTRA_INIT=sqlcipher_extra_init -DSQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown"
  $BatLines = @(
    '@echo off'
    "echo [1/4] Loading VS environment: $VsDevCmd"
    "call `"$VsDevCmd`" -arch=amd64 || exit /b 1"
    "set `"INCLUDE=$OpenSslIncludeShort;%INCLUDE%`""
    "set `"LIB=$OpenSslLibDirShort;%LIB%`""
    "echo [2/4] Entering $SrcDir"
    "cd /d `"$SrcDir`" || exit /b 1"
    'echo Cleaning previous objects...'
    'nmake /f Makefile.msc clean'
    'echo [3/4] Building jimsh0 + sqlite3.c amalgamation'
    'nmake /f Makefile.msc jimsh0.exe'
    'if errorlevel 1 exit /b 1'
    'nmake /f Makefile.msc sqlite3.c USE_AMALGAMATION=1'
    'if errorlevel 1 exit /b 1'
    'if not exist sqlite3.c ('
    '  echo Failed to generate sqlite3.c amalgamation.'
    '  exit /b 1'
    ')'
    'echo [4/4] Building libsqlite3.lib with OpenSSL codec'
    ("nmake /f Makefile.msc libsqlite3.lib USE_AMALGAMATION=1 " +
      "OPTS=`"$Opts`" " +
      "CCOPTS=`"-I$OpenSslIncludeShort`" " +
      "LTLIBPATHS=`"/LIBPATH:$OpenSslLibDirShort`" " +
      "LTLIBS=`"$CryptoLib $SslLib ws2_32.lib crypt32.lib advapi32.lib user32.lib`"")
    'if errorlevel 1 exit /b 1'
    'echo Build finished OK.'
  )
  Set-Content -Path $BuildBat -Value $BatLines -Encoding Ascii

  try {
    cmd /c $BuildBat
    if ($LASTEXITCODE -ne 0) { throw "SQLCipher Windows build failed." }
  } finally {
    Remove-Item -Force $BuildBat -ErrorAction SilentlyContinue
  }
  Write-Host "Compile finished; installing artifact..."

  $Built = @(
    (Join-Path $SrcDir "libsqlite3.lib"),
    (Join-Path $SrcDir "sqlite3.lib"),
    (Join-Path $SrcDir "sqlcipher.lib")
  ) | Where-Object { Test-Path $_ } | Select-Object -First 1
  if (-not $Built) {
    throw "Built library not found under $SrcDir"
  }
  Copy-Item -Force $Built $ArtifactLib
  foreach ($Hdr in @("sqlite3.h", "sqlite3ext.h")) {
    $SrcHdr = Join-Path $SrcDir $Hdr
    if (Test-Path $SrcHdr) {
      Copy-Item -Force $SrcHdr (Join-Path $IncludeDir $Hdr)
    }
  }
  if (-not (Test-Path (Join-Path $IncludeDir "sqlite3.h"))) {
    throw "sqlite3.h missing after Windows build"
  }
}

$LibSha = Get-FileSha256Hex $ArtifactLib
@(
  "$LibSha  sqlcipher.lib"
) | Set-Content -Path (Join-Path $ArtifactDir "SHA256SUMS") -Encoding Ascii

$ReviewedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$ManifestPath = Join-Path $OutRoot "MANIFEST.toml"

# Preserve Linux artifact block if present in an existing manifest.
$LinuxBlock = ""
if (Test-Path $ManifestPath) {
  $Existing = Get-Content -Raw $ManifestPath
  if ($Existing -match '(?s)(\[\[artifact\]\]\s*target = "x86_64-unknown-linux-gnu".*?)(?=\n\[\[artifact\]\]|\z)') {
    $LinuxBlock = $Matches[1].TrimEnd()
  }
}

$Header = @"
schema_version = 1
sqlcipher_version = "$Version"
openssl_fips_note = "OpenSSL via OPENSSL_ROOT_DIR; not FIPS-validated"
reviewed_by = "Gotigin engineering (pinned official SQLCipher $Version self-build)"
reviewed_at_utc = "$ReviewedAt"

"@

$WindowsBlock = @"
[[artifact]]
target = "x86_64-pc-windows-msvc"
path = "windows-x86_64/sqlcipher.lib"
sha256 = "$LibSha"
provenance_url = "$ProvenanceUrl"
"@

$Parts = @($Header.TrimEnd())
if ($LinuxBlock) { $Parts += ""; $Parts += $LinuxBlock }
$Parts += ""
$Parts += $WindowsBlock
($Parts -join "`n") + "`n" | Set-Content -Path $ManifestPath -Encoding Ascii

Write-Host ""
Write-Host "Provisioned Windows SQLCipher ${Version}:"
Write-Host "  $ArtifactLib"
Write-Host "  sha256=$LibSha"
Write-Host "  MANIFEST.toml updated"
Write-Host ""
Write-Host "Next: .\scripts\generate-release-signing-keys.ps1  (if needed)"
Write-Host "Then: .\scripts\build-windows-release.ps1"
