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
extension kubernetes with {
  namespace: 'default'
  kubeConfig: ''
} as k8s

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

@description('Kubernetes namespace Radius will deploy resources into.')
param namespace string = 'trading'

@description('Name of the Radius environment to create/update. Defaults to the namespace so it aligns with the environment the `ada bootstrap` CLI pre-creates via `rad environment create`.')
param environmentName string = namespace

@description('Pre-create the app namespace with the `istio-injection=enabled` label so workloads deployed by Radius receive an Istio sidecar (and inherit the mesh-wide STRICT mTLS policy from the `core` portfolio). Set to false on clusters without Istio.')
param enableIstioInjection bool = true

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
// App namespace (with optional Istio injection label)
// ---------------------------------------------------------------------------

resource appNamespace 'core/Namespace@v1' = {
  metadata: {
    name: namespace
    labels: enableIstioInjection ? {
      'istio-injection': 'enabled'
    } : {}
  }
}

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

resource aksEnv 'Applications.Core/environments@2023-10-01-preview' = {
  name: environmentName
  dependsOn: [
    appNamespace
  ]
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
          templatePath: '${recipeRegistry}/postgres-azure-flex:latest'
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
      // ── Governance — Open Policy Agent (PDP) ───────────────────────────
      'Radius.Resources/governance': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/governance-opa:latest'
        }
      }      // ── Agent guardrails — AGT in-pod sidecar ─────────────────────
      'Radius.Resources/agentGuardrails': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/agent-guardrails-agt:latest'
        }
      }    }
  }
}
