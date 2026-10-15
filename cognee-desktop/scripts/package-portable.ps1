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
  $PortableRuntime = Join-Path $Stage 'runtime'
  $PortablePython = Join-Path $PortableRuntime 'python'
  $PortableVenv = Join-Path $PortableRuntime '.venv'
  $Cache = Join-Path $env:LOCALAPPDATA 'CogneeDesktop\uv-cache'
  New-Item -ItemType Directory -Path $Cache -Force | Out-Null
  $env:UV_PYTHON_INSTALL_DIR = $PortablePython
  $env:UV_CACHE_DIR = $Cache
  $env:UV_PROJECT_ENVIRONMENT = $PortableVenv
  & $Uv python install 3.12.10 --install-dir $PortablePython --no-bin
  if ($LASTEXITCODE -ne 0) { throw 'Could not bundle CPython 3.12.10 in the portable package.' }
  & $Uv sync --locked --no-dev --no-editable --project (Join-Path $PortableRuntime 'pyproject.toml') --python 3.12.10
  if ($LASTEXITCODE -ne 0) { throw 'Could not bundle the locked Cognee dependencies in the portable package.' }
  $PortablePythonExe = Join-Path $PortableVenv 'Scripts/python.exe'
  if (-not (Test-Path -LiteralPath $PortablePythonExe -PathType Leaf)) { throw 'The portable Python environment was not created.' }
  & $PortablePythonExe -I -c 'import fastembed, src'
  if ($LASTEXITCODE -ne 0) { throw 'The bundled Cognee runtime failed its import check.' }
  Remove-Item Env:UV_PYTHON_INSTALL_DIR, Env:UV_CACHE_DIR, Env:UV_PROJECT_ENVIRONMENT -ErrorAction SilentlyContinue
  Get-ChildItem -LiteralPath (Join-Path $Stage 'runtime') -Directory -Filter '__pycache__' -Recurse | Remove-Item -Recurse -Force
  Copy-Item -LiteralPath (Join-Path $AppRoot 'LICENSES.md') -Destination $Stage
  @'
Cognee Desktop 便携版（Windows x64）

使用方法：解压到有写入权限的目录，双击“Cognee Desktop.exe”启动。

本便携包已包含 Python 和 Cognee 运行依赖。首次启动无需下载或安装 Python 依赖。
本便携包免安装；记忆数据、配置和日志保存在当前 Windows 用户的应用数据目录，密钥保存在 Windows 凭据管理器，不随 ZIP 搬运。
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
