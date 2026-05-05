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

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

resource tradingEnv 'Applications.Core/environments@2023-10-01-preview' = {
  name: 'trading'
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
    }
  }
}
