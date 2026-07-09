<#
.SYNOPSIS
Bootstraps the AKS-based Azure workshop environment for solutions 01-04.

.DESCRIPTION
Runs the Azure + Radius setup flow for env-azure-prod in four idempotent stages:
01 creates or updates the Azure resource group, ACR, AKS cluster, and workload
identity app; 02 installs Radius and configures the workspace/environment;
03 imports Radius resource types; 04 publishes recipes to ACR and deploys the
AKS environment definition.

.EXAMPLE
.\infra\azure\deploy-solution-01-04.ps1 `
  -Solutions 01,02,03,04 `
  -SubscriptionId <subscription-id> `
  -Location westeurope `
  -ResourceGroupName rg-adaptive-prod `
  -AksClusterName aks-adaptive-prod `
  -AksNodeCount 2 `
  -AksNodeVmSize Standard_D2s_v5 `
  -AksNodeOsDiskType Managed `
  -AcrName adaptiveappsprodacr `
  -AcrSku Basic `
  -RadiusWorkspaceName ws-azure-prod `
  -RadiusEnvironmentName env-azure-prod `
  -RadiusNamespace env-azure-prod `
  -RadiusGroupName rg-trading `
  -RadiusApplicationDisplayName aks-adaptive-prod-radius-app `
  -RadiusAzureRoleName Owner `
  -RadiusPublicEndpointOverride localhost:8081
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('01', '02', '03', '04')]
    [string[]]$Solutions,

    [Parameter(Mandatory = $true)]
    [string]$SubscriptionId,

    [Parameter(Mandatory = $true)]
    [string]$Location,

    [Parameter(Mandatory = $true)]
    [string]$ResourceGroupName,

    [Parameter(Mandatory = $true)]
    [string]$AksClusterName,

    [Parameter(Mandatory = $true)]
    [int]$AksNodeCount,

    [Parameter(Mandatory = $true)]
    [string]$AksNodeVmSize,

    [Parameter(Mandatory = $true)]
    [ValidateSet('Managed', 'Ephemeral')]
    [string]$AksNodeOsDiskType,

    [Parameter(Mandatory = $true)]
    [string]$AcrName,

    [Parameter(Mandatory = $true)]
    [ValidateSet('Basic', 'Standard', 'Premium')]
    [string]$AcrSku,

    [Parameter(Mandatory = $true)]
    [string]$RadiusWorkspaceName,

    [Parameter(Mandatory = $true)]
    [string]$RadiusEnvironmentName,

    [Parameter(Mandatory = $true)]
    [string]$RadiusNamespace,

    [Parameter(Mandatory = $true)]
    [string]$RadiusGroupName,

    [Parameter(Mandatory = $true)]
    [string]$RadiusApplicationDisplayName,

    [Parameter(Mandatory = $true)]
    [ValidateSet('Owner', 'Contributor')]
    [string]$RadiusAzureRoleName,

    [Parameter(Mandatory = $true)]
    [string]$RadiusPublicEndpointOverride,

    [Parameter(Mandatory = $false)]
    [bool]$EnableIstioInjection = $true,

    [Parameter(Mandatory = $false)]
    [string]$CustomSqlRecipeFilePath,

    [Parameter(Mandatory = $false)]
    [string]$CustomSqlRecipeTag = 'recipes/sql-server:1.0.0'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Note {
    param([string]$Message)
    Write-Host " -> $Message"
}

function Invoke-Cli {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $raw = (& $Executable @Arguments 2>&1) -join "`n"
    $output = $raw.Trim()

    if ($LASTEXITCODE -ne 0) {
        throw "$Executable $($Arguments -join ' ') failed.`n$output"
    }

    return $output
}

function Invoke-Az {
    param([string[]]$CliArgs)
    return Invoke-Cli -Executable 'az' -Arguments (@() + $CliArgs + @('--only-show-errors'))
}

function Invoke-AzJson {
    param([string[]]$CliArgs)

    $raw = Invoke-Cli -Executable 'az' -Arguments (@() + $CliArgs + @('--output', 'json', '--only-show-errors'))
    if (-not $raw) {
        throw "az $($CliArgs -join ' ') returned empty output while JSON was expected."
    }

    $startObject = $raw.IndexOf('{')
    $startArray = $raw.IndexOf('[')
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
        throw "az $($CliArgs -join ' ') did not return JSON.`n$raw"
    }

    return ($raw.Substring($start) | ConvertFrom-Json)
}

function Invoke-Kubectl {
    param([string[]]$CliArgs)
    return Invoke-Cli -Executable 'kubectl' -Arguments $CliArgs
}

function Invoke-Rad {
    param([string[]]$CliArgs)
    return Invoke-Cli -Executable 'rad' -Arguments $CliArgs
}

function ConvertTo-Hashtable {
    param([Parameter(Mandatory = $true)]$Value)

    if ($null -eq $Value) {
        return $null
    }

    if ($Value -is [System.Collections.IDictionary]) {
        $table = @{}
        foreach ($key in $Value.Keys) {
            $table[$key] = ConvertTo-Hashtable -Value $Value[$key]
        }
        return $table
    }

    if ($Value -is [System.Collections.IEnumerable] -and $Value -isnot [string]) {
        $items = @()
        foreach ($item in $Value) {
            $items += ,(ConvertTo-Hashtable -Value $item)
        }
        return $items
    }

    if ($Value.PSObject -and $Value.PSObject.Properties.Count -gt 0) {
        $table = @{}
        foreach ($property in $Value.PSObject.Properties) {
            $table[$property.Name] = ConvertTo-Hashtable -Value $property.Value
        }
        return $table
    }

    return $Value
}

function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
}

function Test-CommandAvailable {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Assert-RequiredCommands {
    foreach ($name in @('az', 'kubectl', 'rad')) {
        if (-not (Test-CommandAvailable -Name $name)) {
            throw "$name is not installed or not available on PATH."
        }
    }
}

function Ensure-RoleAssignment {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PrincipalId,

        [Parameter(Mandatory = $true)]
        [ValidateSet('User', 'Group', 'ServicePrincipal', 'ForeignGroup', 'Device')]
        [string]$PrincipalType,

        [Parameter(Mandatory = $true)]
        [string]$Scope,

        [Parameter(Mandatory = $true)]
        [string]$RoleName
    )

    $count = Invoke-Az -CliArgs @(
        'role', 'assignment', 'list',
        '--assignee-object-id', $PrincipalId,
        '--scope', $Scope,
        '--query', "[?roleDefinitionName=='$RoleName'] | length(@)",
        '-o', 'tsv'
    )

    if ([int]$count -gt 0) {
        Write-Note "$RoleName already assigned at scope $Scope"
        return
    }

    Invoke-Az -CliArgs @(
        'role', 'assignment', 'create',
        '--assignee-object-id', $PrincipalId,
        '--assignee-principal-type', $PrincipalType,
        '--role', $RoleName,
        '--scope', $Scope
    ) | Out-Null

    Write-Note "Assigned $RoleName at scope $Scope"
}

function Get-CurrentPrincipal {
    $account = Invoke-AzJson -CliArgs @('account', 'show')
    $principalType = [string]$account.user.type
    $principalName = [string]$account.user.name

    if ([string]::IsNullOrWhiteSpace($principalType) -or [string]::IsNullOrWhiteSpace($principalName)) {
        throw 'Could not determine the signed-in Azure principal.'
    }

    switch ($principalType) {
        'user' {
            $principalId = Invoke-Az -CliArgs @('ad', 'signed-in-user', 'show', '--query', 'id', '-o', 'tsv')
            return @{
                ObjectId = $principalId
                PrincipalType = 'User'
                Name = $principalName
                TenantId = [string]$account.tenantId
            }
        }
        'servicePrincipal' {
            $principalId = Invoke-Az -CliArgs @('ad', 'sp', 'show', '--id', $principalName, '--query', 'id', '-o', 'tsv')
            return @{
                ObjectId = $principalId
                PrincipalType = 'ServicePrincipal'
                Name = $principalName
                TenantId = [string]$account.tenantId
            }
        }
        default {
            throw "Unsupported Azure principal type '$principalType'."
        }
    }
}

function Get-AksDetails {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ResourceGroup,

        [Parameter(Mandatory = $true)]
        [string]$ClusterName
    )

    return Invoke-AzJson -CliArgs @(
        'aks', 'show',
        '--resource-group', $ResourceGroup,
        '--name', $ClusterName,
        '--query', '{id:id,location:location,oidcIssuer:oidcIssuerProfile.issuerUrl,workloadIdentityEnabled:securityProfile.workloadIdentity.enabled,identityType:identity.type,kubeletObjectId:identityProfile.kubeletidentity.objectId}'
    )
}

function Ensure-ResourceGroup {
    param(
        [string]$Name,
        [string]$AzureLocation
    )

    Write-Step "Ensuring resource group $Name"
    Invoke-Az -CliArgs @('group', 'create', '--name', $Name, '--location', $AzureLocation) | Out-Null
}

function Ensure-Acr {
    param(
        [string]$Name,
        [string]$ResourceGroup,
        [string]$AzureLocation,
        [string]$Sku
    )

    Write-Step "Ensuring ACR $Name"

    try {
        $acr = Invoke-AzJson -CliArgs @(
            'acr', 'show',
            '--name', $Name,
            '--resource-group', $ResourceGroup,
            '--query', '{id:id,loginServer:loginServer,name:name}'
        )
        return $acr
    }
    catch {
        Invoke-Az -CliArgs @(
            'acr', 'create',
            '--name', $Name,
            '--resource-group', $ResourceGroup,
            '--location', $AzureLocation,
            '--sku', $Sku
        ) | Out-Null

        return Invoke-AzJson -CliArgs @(
            'acr', 'show',
            '--name', $Name,
            '--resource-group', $ResourceGroup,
            '--query', '{id:id,loginServer:loginServer,name:name}'
        )
    }
}

function Ensure-AksCluster {
    param(
        [string]$ResourceGroup,
        [string]$ClusterName,
        [string]$AzureLocation,
        [int]$NodeCount,
        [string]$NodeVmSize,
        [string]$NodeOsDiskType
    )

    Write-Step "Ensuring AKS cluster $ClusterName"

    $clusterExists = $true
    try {
        $cluster = Get-AksDetails -ResourceGroup $ResourceGroup -ClusterName $ClusterName
    }
    catch {
        $clusterExists = $false
    }

    if (-not $clusterExists) {
        Invoke-Az -CliArgs @(
            'aks', 'create',
            '--resource-group', $ResourceGroup,
            '--name', $ClusterName,
            '--location', $AzureLocation,
            '--node-count', $NodeCount,
            '--node-vm-size', $NodeVmSize,
            '--node-osdisk-type', $NodeOsDiskType,
            '--enable-addons', 'monitoring',
            '--enable-oidc-issuer',
            '--enable-workload-identity',
            '--generate-ssh-keys'
        ) | Out-Null
    }
    else {
        if (-not [bool]$cluster.workloadIdentityEnabled -or [string]::IsNullOrWhiteSpace([string]$cluster.oidcIssuer)) {
            Invoke-Az -CliArgs @(
                'aks', 'update',
                '--resource-group', $ResourceGroup,
                '--name', $ClusterName,
                '--enable-oidc-issuer',
                '--enable-workload-identity'
            ) | Out-Null
        }
    }

    Invoke-Az -CliArgs @(
        'aks', 'get-credentials',
        '--resource-group', $ResourceGroup,
        '--name', $ClusterName,
        '--overwrite-existing'
    ) | Out-Null

    return Get-AksDetails -ResourceGroup $ResourceGroup -ClusterName $ClusterName
}

function Ensure-RadiusApplicationIdentity {
    param(
        [string]$DisplayName,
        [string]$OidcIssuer,
        [string]$ResourceGroupScope,
        [string]$RoleName,
        [string]$AcrScope
    )

    Write-Step "Ensuring Radius workload identity app $DisplayName"

    $apps = @(Invoke-AzJson -CliArgs @('ad', 'app', 'list', '--display-name', $DisplayName))
    if ($apps.Count -gt 1) {
        throw "Multiple Entra applications matched display name '$DisplayName'. Use a unique display name."
    }

    if ($apps.Count -eq 0) {
        $app = Invoke-AzJson -CliArgs @('ad', 'app', 'create', '--display-name', $DisplayName)
    }
    else {
        $app = $apps[0]
    }

    $clientId = [string]$app.appId
    if ([string]::IsNullOrWhiteSpace($clientId)) {
        throw "Could not resolve appId for Entra application '$DisplayName'."
    }

    $applicationObjectId = Invoke-Az -CliArgs @('ad', 'app', 'show', '--id', $clientId, '--query', 'id', '-o', 'tsv')
    if ([string]::IsNullOrWhiteSpace($applicationObjectId)) {
        throw "Could not resolve object id for Entra application '$DisplayName'."
    }

    $servicePrincipalExists = $true
    try {
        $servicePrincipalObjectId = Invoke-Az -CliArgs @('ad', 'sp', 'show', '--id', $clientId, '--query', 'id', '-o', 'tsv')
    }
    catch {
        $servicePrincipalExists = $false
    }

    if (-not $servicePrincipalExists) {
        Invoke-Az -CliArgs @('ad', 'sp', 'create', '--id', $clientId) | Out-Null
        $servicePrincipalObjectId = Invoke-Az -CliArgs @('ad', 'sp', 'show', '--id', $clientId, '--query', 'id', '-o', 'tsv')
    }

    $credentialDefinitions = @(
        @{ Name = 'radius-applications-rp'; Subject = 'system:serviceaccount:radius-system:applications-rp'; Description = 'Kubernetes service account federated credential for applications-rp' }
        @{ Name = 'radius-bicep-de'; Subject = 'system:serviceaccount:radius-system:bicep-de'; Description = 'Kubernetes service account federated credential for bicep-de' }
        @{ Name = 'radius-ucp'; Subject = 'system:serviceaccount:radius-system:ucp'; Description = 'Kubernetes service account federated credential for ucp' }
        @{ Name = 'radius-dynamic-rp'; Subject = 'system:serviceaccount:radius-system:dynamic-rp'; Description = 'Kubernetes service account federated credential for dynamic-rp' }
    )

    $existingCredentials = @(Invoke-AzJson -CliArgs @('ad', 'app', 'federated-credential', 'list', '--id', $applicationObjectId))
    $tempDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("radius-fedcred-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tempDirectory | Out-Null

    try {
        foreach ($definition in $credentialDefinitions) {
            $alreadyExists = $false
            foreach ($credential in $existingCredentials) {
                if ([string]$credential.name -eq $definition.Name) {
                    $alreadyExists = $true
                    break
                }
            }

            if ($alreadyExists) {
                continue
            }

            $payload = @{
                name = $definition.Name
                issuer = $OidcIssuer
                subject = $definition.Subject
                description = $definition.Description
                audiences = @('api://AzureADTokenExchange')
            }

            $payloadPath = Join-Path $tempDirectory ($definition.Name + '.json')
            $payload | ConvertTo-Json -Depth 4 | Set-Content -Path $payloadPath -Encoding utf8

            Invoke-Az -CliArgs @(
                'ad', 'app', 'federated-credential', 'create',
                '--id', $applicationObjectId,
                '--parameters', "@$payloadPath"
            ) | Out-Null
        }
    }
    finally {
        Remove-Item -Recurse -Force -Path $tempDirectory -ErrorAction SilentlyContinue
    }

    Ensure-RoleAssignment -PrincipalId $servicePrincipalObjectId -PrincipalType 'ServicePrincipal' -Scope $ResourceGroupScope -RoleName $RoleName
    Ensure-RoleAssignment -PrincipalId $servicePrincipalObjectId -PrincipalType 'ServicePrincipal' -Scope $AcrScope -RoleName 'AcrPull'

    return @{
        ClientId = $clientId
        ServicePrincipalObjectId = $servicePrincipalObjectId
    }
}

function Ensure-AcrAccess {
    param(
        [string]$AcrId,
        [string]$KubeletObjectId,
        [hashtable]$CurrentPrincipal,
        [hashtable]$RadiusIdentity
    )

    Write-Step 'Ensuring ACR access'

    Ensure-RoleAssignment -PrincipalId $CurrentPrincipal.ObjectId -PrincipalType $CurrentPrincipal.PrincipalType -Scope $AcrId -RoleName 'AcrPush'

    if ([string]::IsNullOrWhiteSpace($KubeletObjectId)) {
        throw 'Could not resolve the AKS kubelet object id for AcrPull assignment.'
    }

    Ensure-RoleAssignment -PrincipalId $KubeletObjectId -PrincipalType 'ServicePrincipal' -Scope $AcrId -RoleName 'AcrPull'
    Ensure-RoleAssignment -PrincipalId $RadiusIdentity.ServicePrincipalObjectId -PrincipalType 'ServicePrincipal' -Scope $AcrId -RoleName 'AcrPull'
}

function Ensure-RadiusInstalled {
    param([string]$PublicEndpointOverride)

    Write-Step 'Ensuring Radius control plane is installed'

    $radiusInstalled = $false
    try {
        $deployment = Invoke-Kubectl -CliArgs @('get', 'deployment', 'applications-rp', '-n', 'radius-system', '-o', 'name')
        if ($deployment -eq 'deployment.apps/applications-rp') {
            $radiusInstalled = $true
        }
    }
    catch {
        $radiusInstalled = $false
    }

    if (-not $radiusInstalled) {
        Invoke-Rad -CliArgs @(
            'install', 'kubernetes',
            '--set', "rp.publicEndpointOverride=$PublicEndpointOverride",
            '--set', 'global.azureWorkloadIdentity.enabled=true'
        ) | Out-Null
    }

    Invoke-Kubectl -CliArgs @('rollout', 'status', 'deployment/applications-rp', '-n', 'radius-system', '--timeout=10m') | Out-Null
    Invoke-Kubectl -CliArgs @('rollout', 'status', 'deployment/bicep-de', '-n', 'radius-system', '--timeout=10m') | Out-Null
}

function Test-RadListContains {
    param(
        [string[]]$ListArgs,
        [string]$Name
    )

    $output = Invoke-Rad -CliArgs $ListArgs
    $escapedName = [Regex]::Escape($Name)

    foreach ($line in ($output -split "`r?`n")) {
        if ($line -match "(^|\s)$escapedName(\s|$)") {
            return $true
        }
    }

    return $false
}

function Ensure-RadiusWorkspaceAndEnvironment {
    param(
        [string]$WorkspaceName,
        [string]$GroupName,
        [string]$EnvironmentName,
        [string]$Namespace,
        [string]$Subscription,
        [string]$ResourceGroup
    )

    Write-Step "Ensuring Radius workspace $WorkspaceName"
    $context = Invoke-Kubectl -CliArgs @('config', 'current-context')
    Invoke-Rad -CliArgs @('workspace', 'create', 'kubernetes', $WorkspaceName, '--context', $context, '--force') | Out-Null
    Invoke-Rad -CliArgs @('workspace', 'switch', $WorkspaceName) | Out-Null

    Write-Step "Ensuring Radius group $GroupName"
    if (-not (Test-RadListContains -ListArgs @('group', 'list') -Name $GroupName)) {
        Invoke-Rad -CliArgs @('group', 'create', $GroupName) | Out-Null
    }
    Invoke-Rad -CliArgs @('group', 'switch', $GroupName) | Out-Null

    Write-Step "Ensuring Radius environment $EnvironmentName"
    if (-not (Test-RadListContains -ListArgs @('env', 'list') -Name $EnvironmentName)) {
        Invoke-Rad -CliArgs @('env', 'create', $EnvironmentName, '--group', $GroupName, '--kubernetes-namespace', $Namespace) | Out-Null
    }

    Invoke-Rad -CliArgs @(
        'env', 'update', $EnvironmentName,
        '--azure-subscription-id', $Subscription,
        '--azure-resource-group', $ResourceGroup
    ) | Out-Null

    Invoke-Rad -CliArgs @('env', 'switch', $EnvironmentName) | Out-Null
}

function Ensure-RadiusAzureCredential {
    param(
        [string]$ClientId,
        [string]$TenantId
    )

    Write-Step 'Ensuring Radius Azure workload identity credential'
    Invoke-Rad -CliArgs @('credential', 'register', 'azure', 'wi', '--client-id', $ClientId, '--tenant-id', $TenantId) | Out-Null
}

function Ensure-ResourceTypes {
    param(
        [string]$TypesFilePath,
        [string]$ExtensionTargetPath
    )

    Write-Step 'Ensuring Radius resource types are registered'

    $requiredTypes = @(
        'Radius.Resources/postgreSqlDatabases'
        'Radius.Resources/mqttBrokers'
        'Radius.Resources/idProviders'
        'Radius.Resources/workloadIdentities'
        'Radius.Resources/aiModels'
        'Radius.Resources/governance'
        'Radius.Resources/agentGuardrails'
    )

    $shouldImport = $false
    foreach ($typeName in $requiredTypes) {
        if (-not (Test-RadListContains -ListArgs @('resource-type', 'list') -Name $typeName)) {
            $shouldImport = $true
            break
        }
    }

    if ($shouldImport) {
        Invoke-Rad -CliArgs @('resource-type', 'create', '-f', $TypesFilePath) | Out-Null
    }

    Invoke-Rad -CliArgs @('bicep', 'publish-extension', '--from-file', $TypesFilePath, '--target', $ExtensionTargetPath) | Out-Null
}

function Ensure-AcrPublishAuth {
    param(
        [string]$AcrNameToUse,
        [string]$LoginServer
    )

    Write-Step "Ensuring local OCI auth for $LoginServer"

    if (Test-CommandAvailable -Name 'docker') {
        Invoke-Az -CliArgs @('acr', 'login', '--name', $AcrNameToUse) | Out-Null
        return
    }

    $token = Invoke-Az -CliArgs @('acr', 'login', '--name', $AcrNameToUse, '--expose-token', '--query', 'accessToken', '-o', 'tsv')
    if ([string]::IsNullOrWhiteSpace($token)) {
        throw "Could not obtain an ACR access token for $AcrNameToUse."
    }

    $dockerDirectory = Join-Path $env:USERPROFILE '.docker'
    $configPath = Join-Path $dockerDirectory 'config.json'
    $credentials = "00000000-0000-0000-0000-000000000000:$token"
    $auth = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($credentials))

    New-Item -ItemType Directory -Path $dockerDirectory -Force | Out-Null

    $config = @{ auths = @{} }
    if (Test-Path $configPath) {
        $raw = Get-Content -Path $configPath -Raw
        if (-not [string]::IsNullOrWhiteSpace($raw)) {
            $config = ConvertTo-Hashtable -Value ($raw | ConvertFrom-Json)
        }
        if (-not $config.ContainsKey('auths')) {
            $config['auths'] = @{}
        }
    }

    $config['auths'][$LoginServer] = @{ auth = $auth }
    $config | ConvertTo-Json -Depth 10 | Set-Content -Path $configPath -Encoding utf8
}

function Publish-Recipes {
    param(
        [string]$RepoRoot,
        [string]$LoginServer
    )

    Write-Step "Publishing built-in recipes to $LoginServer"

    $recipes = @(
        @{ File = 'radius\recipes\postgres\kubernetes-trading-postgres.bicep'; Tag = 'recipes/postgres:latest' }
        @{ File = 'radius\recipes\postgres\azure-postgresql-flexible-server.bicep'; Tag = 'recipes/postgres-azure-flex:latest' }
        @{ File = 'radius\recipes\mqtt\kubernetes-mosquitto.bicep'; Tag = 'recipes/mqtt:latest' }
        @{ File = 'radius\recipes\mqtt\azure-event-grid.bicep'; Tag = 'recipes/mqtt-azure-event-grid:latest' }
        @{ File = 'radius\recipes\workload-identity\local-noop.bicep'; Tag = 'recipes/workload-identity-local:latest' }
        @{ File = 'radius\recipes\workload-identity\azure-workload-identity.bicep'; Tag = 'recipes/workload-identity-azure:latest' }
        @{ File = 'radius\recipes\idp\kubernetes-keycloak.bicep'; Tag = 'recipes/idp-keycloak:latest' }
        @{ File = 'radius\recipes\ai-agent\kubernetes-kaito.bicep'; Tag = 'recipes/ai-agent-kaito:latest' }
        @{ File = 'radius\recipes\ai-agent\azure-openai.bicep'; Tag = 'recipes/ai-agent-azure-openai:latest' }
        @{ File = 'radius\recipes\governance\kubernetes-opa.bicep'; Tag = 'recipes/governance-opa:latest' }
        @{ File = 'radius\recipes\agent-guardrails\kubernetes-agt-sidecar.bicep'; Tag = 'recipes/agent-guardrails-agt:latest' }
    )

    foreach ($recipe in $recipes) {
        $filePath = Join-Path $RepoRoot $recipe.File
        if (-not (Test-Path $filePath -PathType Leaf)) {
            throw "Recipe file not found: $filePath"
        }

        Invoke-Rad -CliArgs @(
            'bicep', 'publish',
            '--file', $filePath,
            '--target', "br:$LoginServer/$($recipe.Tag)"
        ) | Out-Null
    }
}

function Publish-CustomSqlRecipe {
    param(
        [string]$LoginServer,
        [string]$RecipeFilePath,
        [string]$RecipeTag
    )

    if ([string]::IsNullOrWhiteSpace($RecipeFilePath)) {
        return ''
    }

    if (-not (Test-Path $RecipeFilePath -PathType Leaf)) {
        throw "Custom SQL recipe file not found: $RecipeFilePath"
    }

    Write-Step "Publishing custom sqlDatabases recipe to $LoginServer"
    Invoke-Rad -CliArgs @(
        'bicep', 'publish',
        '--file', $RecipeFilePath,
        '--target', "br:$LoginServer/$RecipeTag"
    ) | Out-Null

    return "$LoginServer/$RecipeTag"
}

function Deploy-AksEnvironmentDefinition {
    param(
        [string]$AksEnvironmentFilePath,
        [string]$GroupName,
        [string]$EnvironmentName,
        [string]$Namespace,
        [string]$Subscription,
        [string]$ResourceGroup,
        [string]$RecipeRegistry,
        [bool]$IstioInjectionEnabled,
        [string]$SqlRecipeTemplatePath
    )

    Write-Step "Deploying Radius environment definition for $EnvironmentName"

    $arguments = @(
        'deploy', $AksEnvironmentFilePath,
        '--group', $GroupName,
        '--environment', $EnvironmentName,
        '--parameters', "environmentName=$EnvironmentName",
        '--parameters', "namespace=$Namespace",
        '--parameters', "azureSubscriptionId=$Subscription",
        '--parameters', "azureResourceGroup=$ResourceGroup",
        '--parameters', "recipeRegistry=$RecipeRegistry",
        '--parameters', "enableIstioInjection=$($IstioInjectionEnabled.ToString().ToLowerInvariant())"
    )

    if (-not [string]::IsNullOrWhiteSpace($SqlRecipeTemplatePath)) {
        $arguments += @('--parameters', "sqlDatabasesRecipeTemplatePath=$SqlRecipeTemplatePath")
    }

    Invoke-Rad -CliArgs $arguments | Out-Null
}

Assert-RequiredCommands

$repoRoot = Get-RepoRoot
$resourceGroupScope = "/subscriptions/$SubscriptionId/resourceGroups/$ResourceGroupName"
$resourceTypesFilePath = Join-Path $repoRoot 'radius\resource-types\types.yaml'
$bicepExtensionTargetPath = Join-Path $repoRoot 'radius\types.tgz'
$aksEnvironmentFilePath = Join-Path $repoRoot 'radius\aks-env.bicep'

if (-not (Test-Path $resourceTypesFilePath -PathType Leaf)) {
    throw "Resource type manifest not found: $resourceTypesFilePath"
}

if (-not (Test-Path $aksEnvironmentFilePath -PathType Leaf)) {
    throw "AKS environment Bicep not found: $aksEnvironmentFilePath"
}

$orderedSolutions = @('01', '02', '03', '04')
$requestedSolutions = New-Object System.Collections.Generic.HashSet[string]
foreach ($solution in $Solutions) {
    [void]$requestedSolutions.Add($solution)
}

Write-Step "Setting Azure subscription to $SubscriptionId"
Invoke-Az -CliArgs @('account', 'set', '--subscription', $SubscriptionId) | Out-Null

$currentPrincipal = Get-CurrentPrincipal
$acr = $null
$aks = $null
$radiusIdentity = $null

foreach ($solution in $orderedSolutions) {
    if (-not $requestedSolutions.Contains($solution)) {
        continue
    }

    switch ($solution) {
        '01' {
            Ensure-ResourceGroup -Name $ResourceGroupName -AzureLocation $Location
            $acr = Ensure-Acr -Name $AcrName -ResourceGroup $ResourceGroupName -AzureLocation $Location -Sku $AcrSku
            $aks = Ensure-AksCluster -ResourceGroup $ResourceGroupName -ClusterName $AksClusterName -AzureLocation $Location -NodeCount $AksNodeCount -NodeVmSize $AksNodeVmSize -NodeOsDiskType $AksNodeOsDiskType
            $radiusIdentity = Ensure-RadiusApplicationIdentity -DisplayName $RadiusApplicationDisplayName -OidcIssuer ([string]$aks.oidcIssuer) -ResourceGroupScope $resourceGroupScope -RoleName $RadiusAzureRoleName -AcrScope ([string]$acr.id)
            Ensure-AcrAccess -AcrId ([string]$acr.id) -KubeletObjectId ([string]$aks.kubeletObjectId) -CurrentPrincipal $currentPrincipal -RadiusIdentity $radiusIdentity
        }
        '02' {
            if ($null -eq $acr) {
                $acr = Ensure-Acr -Name $AcrName -ResourceGroup $ResourceGroupName -AzureLocation $Location -Sku $AcrSku
            }
            if ($null -eq $aks) {
                $aks = Ensure-AksCluster -ResourceGroup $ResourceGroupName -ClusterName $AksClusterName -AzureLocation $Location -NodeCount $AksNodeCount -NodeVmSize $AksNodeVmSize -NodeOsDiskType $AksNodeOsDiskType
            }
            if ($null -eq $radiusIdentity) {
                $radiusIdentity = Ensure-RadiusApplicationIdentity -DisplayName $RadiusApplicationDisplayName -OidcIssuer ([string]$aks.oidcIssuer) -ResourceGroupScope $resourceGroupScope -RoleName $RadiusAzureRoleName -AcrScope ([string]$acr.id)
            }

            Ensure-AcrAccess -AcrId ([string]$acr.id) -KubeletObjectId ([string]$aks.kubeletObjectId) -CurrentPrincipal $currentPrincipal -RadiusIdentity $radiusIdentity
            Ensure-RadiusInstalled -PublicEndpointOverride $RadiusPublicEndpointOverride
            Ensure-RadiusWorkspaceAndEnvironment -WorkspaceName $RadiusWorkspaceName -GroupName $RadiusGroupName -EnvironmentName $RadiusEnvironmentName -Namespace $RadiusNamespace -Subscription $SubscriptionId -ResourceGroup $ResourceGroupName
            Ensure-RadiusAzureCredential -ClientId $radiusIdentity.ClientId -TenantId $currentPrincipal.TenantId
        }
        '03' {
            Invoke-Rad -CliArgs @('workspace', 'switch', $RadiusWorkspaceName) | Out-Null
            Ensure-ResourceTypes -TypesFilePath $resourceTypesFilePath -ExtensionTargetPath $bicepExtensionTargetPath
        }
        '04' {
            if ($null -eq $acr) {
                $acr = Ensure-Acr -Name $AcrName -ResourceGroup $ResourceGroupName -AzureLocation $Location -Sku $AcrSku
            }
            if ($null -eq $aks) {
                $aks = Ensure-AksCluster -ResourceGroup $ResourceGroupName -ClusterName $AksClusterName -AzureLocation $Location -NodeCount $AksNodeCount -NodeVmSize $AksNodeVmSize -NodeOsDiskType $AksNodeOsDiskType
            }
            if ($null -eq $radiusIdentity) {
                $radiusIdentity = Ensure-RadiusApplicationIdentity -DisplayName $RadiusApplicationDisplayName -OidcIssuer ([string]$aks.oidcIssuer) -ResourceGroupScope $resourceGroupScope -RoleName $RadiusAzureRoleName -AcrScope ([string]$acr.id)
            }

            Ensure-AcrAccess -AcrId ([string]$acr.id) -KubeletObjectId ([string]$aks.kubeletObjectId) -CurrentPrincipal $currentPrincipal -RadiusIdentity $radiusIdentity
            Invoke-Rad -CliArgs @('workspace', 'switch', $RadiusWorkspaceName) | Out-Null
            Ensure-AcrPublishAuth -AcrNameToUse $AcrName -LoginServer ([string]$acr.loginServer)
            Publish-Recipes -RepoRoot $repoRoot -LoginServer ([string]$acr.loginServer)
            $customSqlRecipeTemplatePath = Publish-CustomSqlRecipe -LoginServer ([string]$acr.loginServer) -RecipeFilePath $CustomSqlRecipeFilePath -RecipeTag $CustomSqlRecipeTag

            Deploy-AksEnvironmentDefinition -AksEnvironmentFilePath $aksEnvironmentFilePath -GroupName $RadiusGroupName -EnvironmentName $RadiusEnvironmentName -Namespace $RadiusNamespace -Subscription $SubscriptionId -ResourceGroup $ResourceGroupName -RecipeRegistry "$($acr.loginServer)/recipes" -IstioInjectionEnabled $EnableIstioInjection -SqlRecipeTemplatePath $customSqlRecipeTemplatePath
        }
    }
}

Write-Step 'Completed Azure AKS environment bootstrap'
