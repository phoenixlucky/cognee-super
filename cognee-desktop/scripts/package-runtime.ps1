param([string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path)
$ErrorActionPreference = 'Stop'
$AppRoot = Split-Path -Parent $PSScriptRoot
$Source = Join-Path $RepositoryRoot 'cognee-mcp'
$Runtime = Join-Path $AppRoot 'runtime'
if (-not (Test-Path -LiteralPath (Join-Path $Source 'src\server.py'))) { throw "Cognee MCP sources not found: $Source" }
$ResolvedAppRoot = [IO.Path]::GetFullPath($AppRoot).TrimEnd('\') + '\'
$ResolvedRuntime = [IO.Path]::GetFullPath($Runtime)
if (-not $ResolvedRuntime.StartsWith($ResolvedAppRoot, [StringComparison]::OrdinalIgnoreCase)) { throw "Refusing to write outside Cognee Desktop: $ResolvedRuntime" }
foreach ($Name in @('pyproject.toml','uv.lock','README.md')) {
  Copy-Item -LiteralPath (Join-Path $Source $Name) -Destination (Join-Path $Runtime $Name) -Force
}
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'LICENSE') -Destination (Join-Path $Runtime 'Cognee-LICENSE') -Force
$SourceTree = Join-Path $Runtime 'src'
if (Test-Path -LiteralPath $SourceTree) { Remove-Item -LiteralPath $SourceTree -Recurse -Force }
Copy-Item -LiteralPath (Join-Path $Source 'src') -Destination $SourceTree -Recurse
Write-Output "Prepared the locked API-mode Cognee MCP package at $Runtime"
