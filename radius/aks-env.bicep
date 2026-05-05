// aks-env.bicep — Radius Environment definition for AKS / Azure deployments
//
// Declares the Radius Environment with Azure provider scope and registers
// all Recipes using the azure-openai AI backend.
//
// Usage:
//   cd radius/
//   rad deploy aks-env.bicep \
//     --parameters azureSubscriptionId=<subscription-id> \
//     --parameters azureResourceGroup=<resource-group>

extension radius

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

@description('Kubernetes namespace Radius will deploy resources into.')
param namespace string = 'trading'

@description('''
OCI registry path where recipes have been published.
Defaults to the GHCR path populated by the CI pipeline (publish-recipes.yml).
Override only if you have published recipes to a different registry.
''')
param recipeRegistry string = 'ghcr.io/microsoft/adaptive-apps/recipes'

@description('Azure subscription ID. Required — used to scope Azure resource creation.')
param azureSubscriptionId string

@description('Azure resource group name. The resource group must already exist.')
param azureResourceGroup string

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

resource aksEnv 'Applications.Core/environments@2023-10-01-preview' = {
  name: 'aks-trading'
  properties: {
    compute: {
      kind: 'kubernetes'
      namespace: namespace
    }
    providers: {
      azure: {
        scope: '/subscriptions/${azureSubscriptionId}/resourceGroups/${azureResourceGroup}'
      }
    }
    recipes: {
      // ── PostgreSQL ──────────────────────────────────────────────────────
      'Radius.Resources/postgreSqlDatabases': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/postgres:latest'
        }
      }
      // ── MQTT broker — Azure Event Grid MQTT endpoint ───────────────────
      'Radius.Resources/mqttBrokers': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/mqtt-azure-event-grid:latest'
        }
      }
      // ── Workload identity — AKS + Azure federated identity ─────────────
      'Radius.Resources/workloadIdentities': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/workload-identity-azure:latest'
        }
      }
      // ── AI model — Azure OpenAI ─────────────────────────────────────────
      'Radius.Resources/aiModels': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/ai-agent-azure-openai:latest'
        }
      }
    }
  }
}
