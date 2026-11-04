#Requires -Version 5.1
<#
.SYNOPSIS
Configures Radius workload identity using the tenant's trusted AKS issuer.
.EXAMPLE
.\wi-helper-private.ps1 -K8sClusterName $AKS_CLUSTER -AzureResourceGroup $RESOURCE_GROUP -AzureSubscriptionId $AZURE_SUBSCRIPTION -OidcIssuerUrl $AKS_OIDC_ISSUER
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateNotNullOrEmpty()]
    [string] $K8sClusterName,

    [Parameter(Mandatory = $true, Position = 1)]
    [ValidateNotNullOrEmpty()]
    [string] $AzureResourceGroup,

    [Parameter(Mandatory = $true, Position = 2)]
    [ValidateNotNullOrEmpty()]
    [string] $AzureSubscriptionId,

    [Parameter(Mandatory = $true, Position = 3)]
    [ValidateNotNullOrEmpty()]
    [string] $OidcIssuerUrl
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-AzureCli {
    param([string[]] $Arguments)

    $result = & az @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Azure CLI failed (exit code $LASTEXITCODE): az $($Arguments -join ' ')"
    }
    return $result
}

# Keep these tenant-policy values aligned with wi-helper-private.sh.
# See: https://eng.ms/docs/cloud-ai-platform/ciso-organization/iamprotect/enterprise-iam/productivity-environment/tsgs/productivitytenantficpolicies
# Issuer tenant segment -> trusted issuer ID:
# 72f988bf-86f1-41af-91ab-2d7cd011db47 -> 1c91ae20-8465-4475-86d0-17f67445a4d0
# 33e01921-4d64-4f8c-a055-5bdaffd5e33d -> b8745ec0-701f-41b6-b866-76dc4c7c44dd
# cdc5aeea-15c5-4db6-b079-fcadd2505dc2 -> dc33615b-4d50-47cf-9c3c-64cdbbab6c54
# 975f013f-7f24-47e8-a7d3-abc4752bf346 -> f271f2d8-131a-43a4-a669-c4ed8f59bf38
$AksFicIssuerId = '1c91ae20-8465-4475-86d0-17f67445a4d0'
$GraphFicIssuerBind = "https://graph.microsoft.com/beta/identity/federatedIdentityCredentialIssuers/$AksFicIssuerId"

$ApplicationName = "$K8sClusterName-radius-app"
$ApplicationClientId = Invoke-AzureCli -Arguments @(
    'ad', 'app', 'list', '--display-name', $ApplicationName,
    '--query', '[0].appId', '-o', 'tsv'
)

if ([string]::IsNullOrWhiteSpace($ApplicationClientId)) {
    $ApplicationClientId = Invoke-AzureCli -Arguments @(
        'ad', 'app', 'create', '--display-name', $ApplicationName,
        '--service-management-reference', '9b3b24b2-ae90-4395-aba2-47911f2374c2',
        '--query', 'appId', '-o', 'tsv'
    )
}
if ([string]::IsNullOrWhiteSpace($ApplicationClientId)) {
    throw 'Azure CLI did not return an application client ID.'
}

$ApplicationObjectId = Invoke-AzureCli -Arguments @(
    'ad', 'app', 'show', '--id', $ApplicationClientId, '--query', 'id', '-o', 'tsv'
)
if ([string]::IsNullOrWhiteSpace($ApplicationObjectId)) {
    throw 'Azure CLI did not return an application object ID.'
}

function New-RadiusFederatedCredential {
    param([string] $Name, [string] $Subject)

    $url = "https://graph.microsoft.com/beta/applications/$ApplicationObjectId/federatedIdentityCredentials"
    $existingId = Invoke-AzureCli -Arguments @(
        'rest', '--method', 'GET', '--url', $url,
        '--query', "value[?name=='$Name'].id | [0]", '-o', 'tsv'
    )

    # Match the Bash helper: replace existing credentials so issuer updates take effect.
    if (-not [string]::IsNullOrWhiteSpace($existingId)) {
        Invoke-AzureCli -Arguments @(
            'rest', '--method', 'DELETE', '--url', "$url/$existingId", '-o', 'none'
        )
    }

    $body = @{
        name = $Name
        'trustedIssuer@odata.bind' = $GraphFicIssuerBind
        issuer = $OidcIssuerUrl
        subject = $Subject
        audiences = @('api://AzureADTokenExchange')
    } | ConvertTo-Json -Depth 3

    # A UTF-8 file avoids native Windows command-line JSON quoting differences.
    $bodyPath = [System.IO.Path]::GetTempFileName()
    try {
        [System.IO.File]::WriteAllText($bodyPath, $body, [System.Text.UTF8Encoding]::new($false))
        Invoke-AzureCli -Arguments @(
            'rest', '--method', 'POST', '--url', $url,
            '--headers', 'Content-Type=application/json', '--body', "@$bodyPath", '-o', 'none'
        )
    }
    finally {
        Remove-Item -LiteralPath $bodyPath -Force
    }
}

foreach ($serviceAccount in @('applications-rp', 'bicep-de', 'ucp', 'dynamic-rp')) {
    New-RadiusFederatedCredential -Name "radius-$serviceAccount" `
        -Subject "system:serviceaccount:radius-system:$serviceAccount"
}

$ServicePrincipalId = Invoke-AzureCli -Arguments @(
    'ad', 'sp', 'list', '--filter', "appId eq '$ApplicationClientId'",
    '--query', '[0].id', '-o', 'tsv'
)
if ([string]::IsNullOrWhiteSpace($ServicePrincipalId)) {
    Invoke-AzureCli -Arguments @('ad', 'sp', 'create', '--id', $ApplicationClientId, '-o', 'none')
}

Invoke-AzureCli -Arguments @(
    'role', 'assignment', 'create', '--assignee', $ApplicationClientId,
    '--role', 'Owner', '--scope', "/subscriptions/$AzureSubscriptionId/resourceGroups/$AzureResourceGroup",
    '-o', 'none'
)
Write-Host "Configured Radius workload identity. Application client ID: $ApplicationClientId"
