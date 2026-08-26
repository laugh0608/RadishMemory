[CmdletBinding()]
param(
    [string]$BaseRef
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$scriptPath = Join-Path $repoRoot "scripts/check-repo.py"
$arguments = @($scriptPath)

if ($BaseRef) {
    $arguments += @("--base-ref", $BaseRef)
}

$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) {
    $python = Get-Command python3 -ErrorAction SilentlyContinue
}

if ($python) {
    & $python.Source @arguments
} else {
    $py = Get-Command py -ErrorAction SilentlyContinue
    if (-not $py) {
        throw "Python 3 is required to run repository baseline checks."
    }
    & $py.Source -3 @arguments
}

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    throw "Cargo from the pinned Rust toolchain is required to run Rust checks."
}

Push-Location $repoRoot
try {
    & $cargo.Source fmt --all --check
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $cargo.Source clippy --workspace --all-targets --all-features --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $cargo.Source test --workspace --all-targets --all-features --locked
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}
