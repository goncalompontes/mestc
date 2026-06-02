#!/usr/bin/env pwsh
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

Write-Host "=== Building mestc (compiler) ==="
cargo install --path crates/mestc --locked

Write-Host "=== Building mest-lsp (language server) ==="
cargo install --path crates/mest-lsp --locked

Write-Host "=== Done ==="
Write-Host "Binaries installed to ~/.cargo/bin/mest.exe and ~/.cargo/bin/mest-lsp.exe"
