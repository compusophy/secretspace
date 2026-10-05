# Build the page and deploy it to Vercel (production).
# Windows PowerShell 5.1 or PowerShell 7.
#
#   .\scripts\deploy.ps1                               # tabs of one browser link
#   .\scripts\deploy.ps1 -Relay wss://your-relay/ws    # devices link too
#   .\scripts\deploy.ps1 -BuildOnly                    # just build dist\
#
# If scripts are blocked:
#   powershell -ExecutionPolicy Bypass -File scripts\deploy.ps1
#
# Vercel hosts the page, not the relay: the relay holds WebSockets open, so it
# runs elsewhere (Railway, Fly: deploy the Dockerfile; it honours PORT).
# Needs Rust (rustup.rs) and, to deploy, the Vercel CLI logged in. Installs the
# wasm target and the pinned wasm-bindgen CLI if they are missing.
param(
    [string]$Relay = $env:RELAY,
    [switch]$BuildOnly
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not $Relay) { $Relay = 'off' }

# Run a native command; stop if it fails.
function Invoke-Native([string]$What, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "$What failed (exit code $LASTEXITCODE)" }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust is missing: install it from https://rustup.rs, then open a new terminal.'
}

# The wasm-bindgen CLI must match the library version in Cargo.lock exactly.
$lock = [IO.File]::ReadAllText((Join-Path $root 'Cargo.lock'))
if ($lock -match 'name = "wasm-bindgen"\r?\nversion = "([^"]+)"') {
    $need = $Matches[1]
} else {
    throw 'wasm-bindgen is not in Cargo.lock'
}
Invoke-Native 'adding the wasm target' { rustup target add wasm32-unknown-unknown }
$have = ''
if (Get-Command wasm-bindgen -ErrorAction SilentlyContinue) {
    $have = ((wasm-bindgen --version) -split ' ')[1]
}
if ($have -ne $need) {
    Write-Host "installing wasm-bindgen-cli $need (one time, a few minutes)"
    Invoke-Native 'installing wasm-bindgen-cli' { cargo install wasm-bindgen-cli --version $need --locked }
}

# Build: the wasm, its glue, the page.
Invoke-Native 'building the page' { cargo build -p secretspace-web --release --target wasm32-unknown-unknown }
$dist = Join-Path $root 'dist'
if (Test-Path $dist) { Remove-Item $dist -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $dist 'pkg') -Force | Out-Null
Invoke-Native 'generating the wasm glue' {
    wasm-bindgen --target web --no-typescript --out-dir dist/pkg target/wasm32-unknown-unknown/release/secretspace_web.wasm
}
$html = [IO.File]::ReadAllText((Join-Path $root 'web/index.html'))
$tag = '<meta name="relay" content="' + $Relay + '">'
$html = $html -replace '<meta name="relay" content="[^"]*">', $tag
if (-not $html.Contains($tag)) { throw 'could not set the relay in the page' }
[IO.File]::WriteAllText((Join-Path $dist 'index.html'), $html, (New-Object System.Text.UTF8Encoding $false))
Copy-Item (Join-Path $root 'web/vercel.json') $dist
$wasm = Get-Item (Join-Path $dist 'pkg/secretspace_web_bg.wasm')
Write-Host ("dist: wasm {0:N0} KB, relay: {1}" -f ($wasm.Length / 1KB), $Relay)
if ($BuildOnly) { return }

# Deploy from a folder named after the project, keeping its Vercel link
# (.vercel) between deploys, so Vercel names the project secretspace.
if (-not (Get-Command vercel -ErrorAction SilentlyContinue)) {
    throw 'The Vercel CLI is missing: npm i -g vercel, then vercel login.'
}
$stage = Join-Path $root '.deploy/secretspace'
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Get-ChildItem $stage -Force | Where-Object { $_.Name -ne '.vercel' } | Remove-Item -Recurse -Force
Copy-Item (Join-Path $dist '*') $stage -Recurse -Force
Invoke-Native 'deploying' { vercel deploy $stage --prod --yes }
