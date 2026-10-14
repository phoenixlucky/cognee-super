param(
  [string]$BuildDirectory = (Join-Path $PSScriptRoot '..\src-tauri\target\release'),
  [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\packages')
)
$ErrorActionPreference = 'Stop'
$BuildDirectory = [IO.Path]::GetFullPath($BuildDirectory)
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$AppRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$Version = (Get-Content -LiteralPath (Join-Path $AppRoot 'package.json') -Raw | ConvertFrom-Json).version
$Executable = Join-Path $BuildDirectory 'cognee-desktop.exe'
$Uv = Join-Path $BuildDirectory 'uv.exe'
$Runtime = Join-Path $BuildDirectory 'runtime'
foreach ($Path in @($Executable, $Uv, (Join-Path $Runtime 'pyproject.toml'), (Join-Path $Runtime 'uv.lock'))) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Required portable runtime file is missing: $Path" }
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$Stage = Join-Path $OutputDirectory ".portable-stage-$([guid]::NewGuid().ToString('N'))"
$Zip = Join-Path $OutputDirectory "Cognee-Desktop-$Version-windows-x64-portable.zip"
if (Test-Path -LiteralPath $Stage) { throw "Portable staging directory already exists: $Stage" }
New-Item -ItemType Directory -Path $Stage -Force | Out-Null
try {
  Copy-Item -LiteralPath $Executable -Destination (Join-Path $Stage 'Cognee Desktop.exe')
  Copy-Item -LiteralPath $Uv -Destination (Join-Path $Stage 'uv.exe')
  Copy-Item -LiteralPath $Runtime -Destination (Join-Path $Stage 'runtime') -Recurse
  Get-ChildItem -LiteralPath (Join-Path $Stage 'runtime') -Directory -Filter '__pycache__' -Recurse | Remove-Item -Recurse -Force
  Copy-Item -LiteralPath (Join-Path $AppRoot 'LICENSES.md') -Destination $Stage
  @'
Cognee Desktop 便携版（Windows x64）

使用方法：解压到有写入权限的目录，双击“Cognee Desktop.exe”启动。

首次准备 Cognee 运行环境需要联网，并需要为 Python 和依赖包预留数 GiB 空间。
本便携包免安装，但记忆数据、运行时、配置和日志保存在当前 Windows 用户的应用数据目录；密钥保存在 Windows 凭据管理器，不随 ZIP 搬运。
需要可用的 Microsoft Edge WebView2 Runtime。
'@ | Set-Content -LiteralPath (Join-Path $Stage '使用说明.txt') -Encoding utf8
  if (Test-Path -LiteralPath $Zip) { Remove-Item -LiteralPath $Zip -Force }
  Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Zip -CompressionLevel Optimal
  Write-Output "Created portable package: $Zip"
} finally {
  $ResolvedStage = [IO.Path]::GetFullPath($Stage)
  $ResolvedOutput = $OutputDirectory.TrimEnd('\') + '\'
  if (-not $ResolvedStage.StartsWith($ResolvedOutput, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to remove staging directory outside output directory: $ResolvedStage"
  }
  Remove-Item -LiteralPath $ResolvedStage -Recurse -Force -ErrorAction SilentlyContinue
}
