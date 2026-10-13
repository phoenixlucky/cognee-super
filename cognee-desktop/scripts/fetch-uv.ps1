param([switch]$VerifyOnly)
$ErrorActionPreference = 'Stop'
$Version = '0.11.19'
$ExpectedArchiveSha256 = '1665fc8e37b5d70a134820d6d7891747471a2ac8bc940ee7af0b69fd03b28d61'
$ExpectedBinarySha256 = 'cd628b46729d01ad110146a647a633a6e5de0e091d73db46afaeee6fc b4ba648'.Replace(' ', '')
$Root = Split-Path -Parent $PSScriptRoot
$Output = Join-Path $Root 'resources\uv\uv.exe'
if ($VerifyOnly) {
  if (-not (Test-Path -LiteralPath $Output)) { throw "Missing uv executable: $Output" }
  $Actual = (Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($Actual -ne $ExpectedBinarySha256) { throw "uv.exe SHA-256 mismatch: $Actual" }
  exit 0
}
$Temp = Join-Path ([IO.Path]::GetTempPath()) "cognee-uv-$Version.zip"
$Extract = Join-Path ([IO.Path]::GetTempPath()) "cognee-uv-$Version"
$Url = "https://github.com/astral-sh/uv/releases/download/$Version/uv-x86_64-pc-windows-msvc.zip"
try {
  Invoke-WebRequest -Uri $Url -OutFile $Temp
  $Actual = (Get-FileHash -LiteralPath $Temp -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($Actual -ne $ExpectedArchiveSha256) { throw "uv archive SHA-256 mismatch: $Actual" }
  if (Test-Path -LiteralPath $Extract) { Remove-Item -LiteralPath $Extract -Recurse -Force }
  Expand-Archive -LiteralPath $Temp -DestinationPath $Extract -Force
  $Candidate = Get-ChildItem -LiteralPath $Extract -Filter uv.exe -Recurse | Select-Object -First 1
  if (-not $Candidate) { throw 'uv.exe was not found in the verified archive' }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Output) | Out-Null
  Copy-Item -LiteralPath $Candidate.FullName -Destination $Output -Force
  if ((Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedBinarySha256) { throw 'Extracted uv.exe SHA-256 mismatch' }
} finally {
  Remove-Item -LiteralPath $Temp -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $Extract -Recurse -Force -ErrorAction SilentlyContinue
}
