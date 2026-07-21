#Requires -Version 5.1
<#
.SYNOPSIS
  Generate Windows Authenticode signing material for ROS release packaging.

.DESCRIPTION
  Creates a code-signing certificate (PKCS#12) under secrets/signing/windows/.
  Also imports/creates the shared GnuPG release key when the Linux-generated
  secret is already present, or creates a Windows-side GnuPG key if gpg exists
  and no secret is present yet.

  Target OS for the product itself: Windows 10 and later.
#>
param(
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$SigningRoot = Join-Path $Root "secrets\signing"
$WindowsDir = Join-Path $SigningRoot "windows"
$GpgDir = Join-Path $SigningRoot "gpg"
$PubKeyDir = Join-Path $Root "ros-website\public\keys"
$PfxPath = Join-Path $WindowsDir "ros-codesign.pfx"
$PasswordPath = Join-Path $WindowsDir "ros-codesign.password"
$PubKeyPath = Join-Path $PubKeyDir "gotigin-ros-release.pub.asc"
$GpgSec = Join-Path $GpgDir "gotigin-ros-release.sec"
$GpgPub = Join-Path $GpgDir "gotigin-ros-release.pub"

New-Item -ItemType Directory -Force -Path $WindowsDir | Out-Null
New-Item -ItemType Directory -Force -Path $GpgDir | Out-Null
New-Item -ItemType Directory -Force -Path $PubKeyDir | Out-Null

if (-not $Force -and (Test-Path $PfxPath)) {
  throw "Refusing to overwrite existing $PfxPath. Pass -Force to rotate."
}

# Cryptographically random PFX password (stored next to the PFX; both gitignored).
$Bytes = New-Object byte[] 32
[System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($Bytes)
$PasswordPlain = [Convert]::ToBase64String($Bytes) -replace '[/+=]', 'A'
$SecurePassword = ConvertTo-SecureString -String $PasswordPlain -Force -AsPlainText

$Cert = New-SelfSignedCertificate `
  -Type CodeSigningCert `
  -Subject "CN=Gotigin Restaurant Operating System, O=Gotigin, C=IN" `
  -KeyAlgorithm RSA `
  -KeyLength 4096 `
  -HashAlgorithm SHA256 `
  -CertStoreLocation "Cert:\CurrentUser\My" `
  -KeyExportPolicy Exportable `
  -NotAfter (Get-Date).AddYears(30) `
  -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3")

Export-PfxCertificate -Cert $Cert -FilePath $PfxPath -Password $SecurePassword | Out-Null
Set-Content -Path $PasswordPath -Value $PasswordPlain -NoNewline
# Remove from CurrentUser store after export; builds import from the PFX file.
Remove-Item "Cert:\CurrentUser\My\$($Cert.Thumbprint)" -Force

Write-Host "Windows Authenticode PFX : $PfxPath"
Write-Host "Windows PFX password file: $PasswordPath"

$Gpg = Get-Command gpg -ErrorAction SilentlyContinue
if ($null -eq $Gpg) {
  Write-Host @"

GnuPG (gpg) was not found on PATH.
Install Gpg4win, then either:
  1) Copy secrets/signing/gpg/ from the Linux machine that ran generate-release-signing-keys.sh
  2) Re-run this script after gpg is available to create the download-verification key
"@
  return
}

if ((Test-Path $GpgSec) -and -not $Force) {
  Write-Host "GnuPG secret already present at $GpgSec (leaving unchanged)."
  if (-not (Test-Path $PubKeyPath) -and (Test-Path $GpgPub)) {
    Copy-Item $GpgPub $PubKeyPath -Force
  }
  return
}

$GpgHome = Join-Path $env:TEMP ("ros-gpg-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $GpgHome | Out-Null
try {
  $env:GNUPGHOME = $GpgHome
  $Batch = Join-Path $GpgHome "batch"
  @"
%echo Generating Gotigin ROS release key
Key-Type: RSA
Key-Length: 4096
Key-Usage: sign
Name-Real: Gotigin ROS Release
Name-Email: ros-release@gotigin.com
Expire-Date: 0
%no-protection
%commit
%echo done
"@ | Set-Content -Path $Batch -Encoding Ascii

  & gpg --batch --generate-key $Batch
  $Fpr = (& gpg --list-secret-keys --with-colons | Where-Object { $_ -like "fpr:*" } | Select-Object -First 1)
  if (-not $Fpr) { throw "Failed to read generated GnuPG fingerprint." }
  $Fingerprint = ($Fpr -split ":")[9]
  & gpg --batch --export-secret-keys --armor $Fingerprint | Set-Content -Path $GpgSec -Encoding Ascii
  & gpg --batch --export --armor $Fingerprint | Set-Content -Path $GpgPub -Encoding Ascii
  Copy-Item $GpgPub $PubKeyPath -Force
  Write-Host "GnuPG secret : $GpgSec"
  Write-Host "GnuPG public : $PubKeyPath"
}
finally {
  Remove-Item -Recurse -Force $GpgHome -ErrorAction SilentlyContinue
  Remove-Item Env:GNUPGHOME -ErrorAction SilentlyContinue
}
