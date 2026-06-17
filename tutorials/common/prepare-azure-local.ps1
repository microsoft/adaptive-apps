[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ResourceGroup,

    [Parameter(Mandatory = $true)]
    [string]$ClusterName,

    [string]$AcrName,
    [string]$KeyVaultName,
    [string]$StorageAccountName
)

$ErrorActionPreference = 'Stop'

function Invoke-Az {
    param([string[]]$CliArgs)
    $cmd = @() + $CliArgs + @('--only-show-errors')
    $raw = (& az @cmd 2>&1) -join "`n"
    $output = $raw.Trim()

    if ($LASTEXITCODE -ne 0) {
        throw "az $($CliArgs -join ' ') failed.`n$output"
    }

    return $output
}

function Invoke-AzJson {
    param([string[]]$CliArgs)

    $cmd = @() + $CliArgs + @('--output', 'json', '--only-show-errors')
    $raw = (& az @cmd 2>&1) -join "`n"
    $trimmed = $raw.Trim()

    if ($LASTEXITCODE -ne 0) {
        throw "az $($CliArgs -join ' ') failed.`n$trimmed"
    }

    if (-not $trimmed) {
        throw 'Azure CLI returned empty output while JSON was expected.'
    }

    $startObject = $trimmed.IndexOf('{')
    $startArray = $trimmed.IndexOf('[')
    $start = -1

    if ($startObject -ge 0 -and $startArray -ge 0) {
        $start = [Math]::Min($startObject, $startArray)
    }
    elseif ($startObject -ge 0) {
        $start = $startObject
    }
    elseif ($startArray -ge 0) {
        $start = $startArray
    }

    if ($start -lt 0) {
        throw "Azure CLI did not return JSON. Output: $trimmed"
    }

    $json = $trimmed.Substring($start)
    return ($json | ConvertFrom-Json)
}

function Ensure-RoleAssignment {
    param(
        [string]$PrincipalId,
        [string]$Scope,
        [string]$RoleName
    )

    if ([string]::IsNullOrWhiteSpace($Scope)) {
        throw "Cannot assign role '$RoleName' because scope is empty."
    }

    $count = Invoke-Az -CliArgs @(
        'role', 'assignment', 'list',
        '--assignee-object-id', $PrincipalId,
        '--scope', $Scope,
        '--query', "[?roleDefinitionName=='$RoleName'] | length(@)",
        '-o', 'tsv'
    )

    if ([int]$count -lt 1) {
        Invoke-Az -CliArgs @(
            'role', 'assignment', 'create',
            '--assignee-object-id', $PrincipalId,
            '--assignee-principal-type', 'ServicePrincipal',
            '--role', $RoleName,
            '--scope', $Scope
        ) | Out-Null
        Write-Host "Created role assignment: $RoleName"
    }
    else {
        Write-Host "Role assignment already exists: $RoleName"
    }
}

if (-not (Get-Command az -ErrorAction SilentlyContinue)) {
    throw 'az CLI is not installed or not in PATH.'
}

if (-not (Get-Command kubectl -ErrorAction SilentlyContinue)) {
    throw 'kubectl is not installed or not in PATH.'
}

$null = Invoke-AzJson -CliArgs @('account', 'show')

$connected = Invoke-AzJson -CliArgs @(
    'connectedk8s', 'show',
    '--resource-group', $ResourceGroup,
    '--name', $ClusterName
)

if (-not $connected.id) {
    throw 'Could not resolve connected cluster resource id.'
}

$subscriptionId = ($connected.id -split '/')[2]
$location = $connected.location
$clusterPrincipalId = $connected.identity.principalId

if (-not $subscriptionId) {
    throw 'Could not determine subscription id from connected cluster.'
}

if (-not $location) {
    throw 'Could not determine location from connected cluster.'
}

if (-not $clusterPrincipalId) {
    throw 'Could not determine cluster principal id from connected cluster identity.'
}

Invoke-Az -CliArgs @('account', 'set', '--subscription', $subscriptionId) | Out-Null

# Simple deterministic defaults. Override with parameters if needed.
$base = (($ResourceGroup + $ClusterName).ToLower() -replace '[^a-z0-9]', '')
if ($base.Length -lt 8) {
    $base = ($base + 'adaptiveapps')
}

if (-not $AcrName) {
    $AcrName = ($base + 'acr')
    if ($AcrName.Length -gt 50) { $AcrName = $AcrName.Substring(0, 50) }
}

if (-not $KeyVaultName) {
    $kvBase = $base
    if ($kvBase.Length -gt 20) { $kvBase = $kvBase.Substring(0, 20) }
    $KeyVaultName = "$kvBase-kv"
}

if (-not $StorageAccountName) {
    $saBase = ($base + 'stg')
    if ($saBase.Length -gt 24) { $saBase = $saBase.Substring(0, 24) }
    $StorageAccountName = $saBase
}

Write-Host "Using subscription: $subscriptionId"
Write-Host "Using location: $location"
Write-Host "ACR: $AcrName"
Write-Host "Key Vault: $KeyVaultName"
Write-Host "Storage: $StorageAccountName"

# Ensure resource group exists.
$rgExists = Invoke-Az -CliArgs @('group', 'exists', '--name', $ResourceGroup)
if ($rgExists -ne 'true') {
    Invoke-Az -CliArgs @('group', 'create', '--name', $ResourceGroup, '--location', $location) | Out-Null
    Write-Host "Created resource group: $ResourceGroup"
}

# Ensure ACR exists.
$acrId = $null
try {
    $acrId = Invoke-Az -CliArgs @('acr', 'show', '--name', $AcrName, '--resource-group', $ResourceGroup, '--query', 'id', '-o', 'tsv')
    Write-Host 'ACR already exists.'
}
catch {
    Invoke-Az -CliArgs @(
        'acr', 'create',
        '--resource-group', $ResourceGroup,
        '--name', $AcrName,
        '--sku', 'Basic',
        '--location', $location
    ) | Out-Null
    $acrId = Invoke-Az -CliArgs @('acr', 'show', '--name', $AcrName, '--resource-group', $ResourceGroup, '--query', 'id', '-o', 'tsv')
    Write-Host 'Created ACR.'
}

# Ensure Key Vault exists and uses RBAC auth.
$keyVaultId = $null
try {
    $keyVaultId = Invoke-Az -CliArgs @('keyvault', 'show', '--name', $KeyVaultName, '--resource-group', $ResourceGroup, '--query', 'id', '-o', 'tsv')
    Write-Host 'Key Vault already exists.'
}
catch {
    Invoke-Az -CliArgs @(
        'keyvault', 'create',
        '--resource-group', $ResourceGroup,
        '--name', $KeyVaultName,
        '--location', $location,
        '--enable-rbac-authorization', 'true'
    ) | Out-Null
    $keyVaultId = Invoke-Az -CliArgs @('keyvault', 'show', '--name', $KeyVaultName, '--resource-group', $ResourceGroup, '--query', 'id', '-o', 'tsv')
    Write-Host 'Created Key Vault.'
}

# Ensure Storage account exists.
$storageId = $null
try {
    $storageId = Invoke-Az -CliArgs @('storage', 'account', 'show', '--name', $StorageAccountName, '--resource-group', $ResourceGroup, '--query', 'id', '-o', 'tsv')
    Write-Host 'Storage account already exists.'
}
catch {
    Invoke-Az -CliArgs @(
        'storage', 'account', 'create',
        '--resource-group', $ResourceGroup,
        '--name', $StorageAccountName,
        '--location', $location,
        '--sku', 'Standard_LRS'
    ) | Out-Null
    $storageId = Invoke-Az -CliArgs @('storage', 'account', 'show', '--name', $StorageAccountName, '--resource-group', $ResourceGroup, '--query', 'id', '-o', 'tsv')
    Write-Host 'Created storage account.'
}

Ensure-RoleAssignment -PrincipalId $clusterPrincipalId -Scope $acrId -RoleName 'AcrPull'
Ensure-RoleAssignment -PrincipalId $clusterPrincipalId -Scope $keyVaultId -RoleName 'Key Vault Secrets User'
Ensure-RoleAssignment -PrincipalId $clusterPrincipalId -Scope $storageId -RoleName 'Storage Blob Data Contributor'

Write-Host ''
Write-Host 'Completed. Resources and role assignments are in place.' -ForegroundColor Green
Write-Host 'If you use Arc proxy access, keep this running in another terminal:'
Write-Host "az connectedk8s proxy -n $ClusterName -g $ResourceGroup"
