param([Parameter(Mandatory=$true)][string]$InstallerPath)
$ErrorActionPreference = 'Stop'
if (-not $env:COGNEE_SIGNING_CERT_BASE64 -or -not $env:COGNEE_SIGNING_CERT_PASSWORD) {
  Write-Warning 'No signing certificate is configured. The installer will be unsigned.'
  exit 0
}
$CertPath = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString() + '.pfx')
try {
  [IO.File]::WriteAllBytes($CertPath, [Convert]::FromBase64String($env:COGNEE_SIGNING_CERT_BASE64))
  $signtool = Get-Command signtool.exe -ErrorAction Stop
  & $signtool.Source sign /fd SHA256 /f $CertPath /p $env:COGNEE_SIGNING_CERT_PASSWORD /tr http://timestamp.digicert.com /td SHA256 $InstallerPath
  if ($LASTEXITCODE -ne 0) { throw "signtool exited with $LASTEXITCODE" }
} finally { Remove-Item -LiteralPath $CertPath -Force -ErrorAction SilentlyContinue }
