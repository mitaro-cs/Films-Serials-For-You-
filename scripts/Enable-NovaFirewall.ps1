# Run as Administrator. Allows only Nova on the private local network.
# Does not disable the firewall, Defender or organization policies.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Program,
    [ValidateRange(1, 65535)][int]$Port = 17480,
    [switch]$Undo
)
$ErrorActionPreference = 'Stop'
$ruleName = 'Nova-Private-LAN'
if ($Undo) {
    Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    Write-Host 'Nova rule removed.'
    return
}
$resolved = (Resolve-Path -LiteralPath $Program).Path
if ([IO.Path]::GetExtension($resolved) -ne '.exe') { throw 'Program must be an executable.' }
if ([IO.Path]::GetFileName($resolved) -notin @('Nova.exe', 'fsfy-desktop.exe', 'fsfy.exe')) {
    throw 'Choose the Nova application executable, not the installer or a test executable.'
}
$backup = Join-Path $PSScriptRoot ('firewall-backup-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.wfw')
& netsh advfirewall export $backup
if ($LASTEXITCODE -ne 0) { throw 'Could not back up firewall settings.' }
if (Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue) {
    throw 'Nova rule already exists. Use -Undo before replacing it.'
}
New-NetFirewallRule -Name $ruleName -DisplayName 'Nova - private local network' `
    -Direction Inbound -Action Allow -Enabled True -Profile Private `
    -Program $resolved -Protocol TCP -LocalPort $Port -RemoteAddress LocalSubnet
Write-Host "Created a scoped Nova rule. Backup: $backup"
