// local-env.bicep — Radius Environment definition for local k3s / dev deployments
//
// Declares the Radius Environment and registers all Recipes using Kaito for AI.
// Deploy this file AFTER publishing recipes to your OCI registry (see README.md).
// For AKS / Azure deployments, use aks-env.bicep instead.
//
// Usage:
//   cd radius/
//   rad deploy local-env.bicep

extension radius
extension kubernetes with {
  namespace: 'default'
  kubeConfig: ''
} as k8s

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

@description('Kubernetes namespace Radius will deploy resources into.')
param namespace string = 'env-local-prod'

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

resource localEnv 'Applications.Core/environments@2023-10-01-preview' = {
  name: environmentName
  dependsOn: [
    appNamespace
  ]
  properties: {
    compute: {
      kind: 'kubernetes'
      namespace: namespace
    }
    providers: {}
    recipes: {
      // ── PostgreSQL ──────────────────────────────────────────────────────
      'Radius.Resources/postgreSqlDatabases': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/postgres:latest'
        }
      }
      // ── MQTT broker ─────────────────────────────────────────────────────
      'Radius.Resources/mqttBrokers': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/mqtt:latest'
        }
      }
      // ── Identity provider — Keycloak (OIDC) ───────────────────────────
      'Radius.Resources/idProviders': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/idp-keycloak:latest'
        }
      }
      // ── Workload identity — local no-op ────────────────────────────────
      'Radius.Resources/workloadIdentities': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/workload-identity-local:latest'
        }
      }
      // ── AI model — Kaito (in-cluster LLM) ──────────────────────────────
      'Radius.Resources/aiModels': {
        default: {
          templateKind: 'bicep'
          templatePath: '${recipeRegistry}/ai-agent-kaito:latest'
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
