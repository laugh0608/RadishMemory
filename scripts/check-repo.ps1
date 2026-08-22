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
if ($python) {
    & $python.Source @arguments
    exit $LASTEXITCODE
}

$python3 = Get-Command python3 -ErrorAction SilentlyContinue
if ($python3) {
    & $python3.Source @arguments
    exit $LASTEXITCODE
}

$py = Get-Command py -ErrorAction SilentlyContinue
if ($py) {
    & $py.Source -3 @arguments
    exit $LASTEXITCODE
}

throw "Python 3 is required to run repository baseline checks."
