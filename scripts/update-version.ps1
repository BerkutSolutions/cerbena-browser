param(
    [string]$Version = ""
)

& (Join-Path $PSScriptRoot "update-version-core.ps1") @PSBoundParameters
