// app.bicep — Radius application model for adaptive-apps
//
// Deploys the full stock-trading simulator on Kubernetes via Radius:
//   • Radius.Resources/postgreSqlDatabases  → PostgreSQL 16 (with trading schema)
//   • Radius.Resources/mqttBrokers          → Eclipse Mosquitto 2 (MQTT + WS)
//   • Radius.Resources/idProviders          → OIDC identity provider (Keycloak by default)
//   • Radius.Resources/aiModels             → AI inference endpoint (only when aiProvider=='local')
//   • Radius.Resources/governance           → Mesh-layer PDP (e.g. OPA + Envoy ext_authz; opt-in)
//   • Radius.Resources/agentGuardrails      → In-pod agent governance sidecar (opt-in, only when AI is enabled)
//   • Applications.Core/containers          → backend (.NET 8), ai-agent (.NET 8), frontend (Node)
//
// AI inference settings:
//   • When aiProvider == 'local', a Radius.Resources/aiModels resource is provisioned by
//     a Recipe and its connection values are auto-injected into the ai-agent container.
//   • Otherwise, the aiProvider / aiEndpoint / aiModelName / aiApiKey parameters are
//     injected directly into the ai-agent container (no Recipe required).
//
// Agent guardrails (opt-in via enableAgentGuardrails=true; requires aiProvider != ''):
//   • The agentGuardrails Recipe stages a policy ConfigMap and emits coordinates
//     (image, ports, ConfigMap name). The ai-agent containers then inject the
//     sidecar via runtimes.kubernetes.pod using DETERMINISTIC NAMES derived from
//     the same resource name. The containers do NOT read .properties of the
//     conditional guardrails resource — that would create a conditional
//     dependency edge and trip the Deployment Engine in Radius v0.57.x.
//
// BEFORE DEPLOYING this file you must:
//   1. Generate & register the Bicep extension (see README.md)
//   2. Register the Recipes in your Radius Environment

extension radius
extension radiusResources

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

@description('The ID of your Radius Environment. Injected automatically by the rad CLI.')
param environment string

@description('Container image registry prefix, e.g. "ghcr.io/myorg" or "myregistry.azurecr.io/adaptive-apps".')
param imageRegistry string = 'ghcr.io/microsoft/adaptive-apps'

@description('Container image tag for all three application services.')
param imageTag string = 'latest'

@description('Username for the local (password-based) frontend login.')
param authUsername string = 'admin'

@description('Password for the local frontend login.')
@secure()
param authPassword string

@description('Secret used to sign Express session cookies. Defaults to a value derived from the environment ID.')
@secure()
#disable-next-line secure-parameter-default
param sessionSecret string = uniqueString(environment, 'session')

@description('OIDC client ID used by frontend to enable Keycloak/OIDC login. Leave empty to disable OIDC login button.')
param oidcClientId string = ''

@description('OIDC client secret for the frontend OIDC client.')
@secure()
param oidcClientSecret string = ''

@description('Public base URL where users access the frontend (used to build OIDC callback URL when needed).')
param appBaseUrl string = 'http://localhost:3000'

@description('Optional browser-facing OIDC authorization endpoint (use when OIDC auth endpoint is internal cluster DNS).')
param oidcBrowserAuthEndpoint string = ''

@description('Optional OIDC issuer override (must match the issuer claim in ID tokens).')
param oidcIssuerOverride string = ''

@description('Client ID of the pre-provisioned managed identity for the backend workload (created by app-wi-setup.sh).')
param backendClientId string = ''

@description('Client ID of the pre-provisioned managed identity for the frontend workload (created by app-wi-setup.sh).')
param frontendClientId string = ''

@description('Kubernetes service account used by workload identity binding.')
param workloadIdentityServiceAccountName string = 'default'

@description('Entra tenant ID used by workload identity resources in Azure environments.')
param workloadIdentityTenantId string = ''

@description('Optional OIDC authorization endpoint override. When empty, derived from oidcIssuer.')
param oidcAuthEndpoint string = ''

@description('Optional OIDC token endpoint override. When empty, derived from oidcIssuer.')
param oidcTokenEndpoint string = ''

@description('Optional OIDC user info endpoint override. When empty, derived from oidcIssuer.')
param oidcUserInfoEndpoint string = ''

@description('OIDC issuer URL (e.g., https://keycloak.example.com/realms/master). Used if oidcIssuerOverride is not set.')
param oidcIssuer string = ''

@description('AI provider mode understood by the ai-agent container: "openai" (OpenAI API), "azure-key" (Azure OpenAI with API key), "azure" (Azure OpenAI with managed identity), "local" (in-cluster LLM). Leave empty when AI is not used.')
param aiProvider string = ''

@description('Base URL for the AI inference API. For OpenAI: https://api.openai.com/v1 (leave empty to use default). For Azure OpenAI: https://<account>.openai.azure.com/ (base URL only, without /openai/deployments). For local: http://kaito-svc/v1. Leave empty when AI is not used.')
param aiEndpoint string = ''

@description('Model or deployment name for the AI inference API (e.g. gpt-4o, llama-3.1-8b-instruct). Leave empty when AI is not used.')
param aiModelName string = ''

@description('API key for the AI inference endpoint. Leave empty for local/unauthenticated endpoints.')
@secure()
param aiApiKey string = ''

@description('''
Model hint for the AI Recipe (used only when aiProvider == 'local').
  Kaito (kubernetes-kaito.bicep)       : a Kaito preset, e.g. llama-3.1-8b-instruct
  Azure OpenAI (azure-openai.bicep)    : a model name to deploy, e.g. gpt-4o
The Recipe uses this value when provisioning the inference backend.
''')
param aiModel string = 'qwen2.5-coder-7b-instruct'

@description('OTLP collector endpoint for sending telemetry (e.g., http://otel-collector.core:4318). Provided by the portfolio/environment.')
param otelCollectorEndpoint string = ''

@description('Enable Istio sidecar injection for all containers in the app (requires Istio to be installed in the cluster).')
param enableIstioInjection bool = true

@description('Deploy a Radius.Resources/governance resource (policy decision point) for the application. Set to true to migrate the legacy OPA workload from the `ent` Helm portfolio onto the portable Radius recipe.')
param enableGovernance bool = false

@description('Enforcement mode passed to the governance recipe. Recipes may interpret this differently (e.g. OPA decision-log only vs. denying responses).')
@allowed([
  'enforce'
  'audit'
  'dryrun'
])
param governanceMode string = 'enforce'

@description('When true (and enableGovernance is true), the governance recipe also registers itself as an Istio mesh extensionProvider so AuthorizationPolicy resources with `action: CUSTOM` can delegate to the PDP. Requires Istio to be installed in the cluster.')
param governanceIstioIntegration bool = true

@description('Deploy a Radius.Resources/agentGuardrails resource and inject the Agent Governance Toolkit (AGT) sidecar into the ai-agent pod. Application-layer, in-pod governance for the LLM agent (prompt-injection scanning, governed tool execution). Independent of `enableGovernance` (mesh-layer authz). Requires a non-empty `aiProvider`.')
param enableAgentGuardrails bool = false

@description('Enforcement mode for the agent guardrails sidecar. Surfaced as the AGT_MODE env var on the sidecar container.')
@allowed([
  'enforce'
  'audit'
  'dryrun'
])
param agentGuardrailsMode string = 'enforce'

@description('Inline policy bundle (YAML) handed to the agent guardrails sidecar. Leave empty to use the recipe default-allow placeholder.')
param agentGuardrailsPolicies string = ''

@description('Override the sidecar container image. When empty the recipe default (the Microsoft-published `ghcr.io/microsoft/agentmesh/governance-sidecar` image) is used.')
param agentGuardrailsImage string = ''

var effectiveOidcIssuer = oidcIssuerOverride != '' ? oidcIssuerOverride : oidcIssuer
var issuerBaseForDerivedEndpoints = endsWith(effectiveOidcIssuer, '/')
  ? substring(effectiveOidcIssuer, 0, max(length(effectiveOidcIssuer) - 1, 0))
  : effectiveOidcIssuer
var effectiveOidcAuthEndpoint = oidcAuthEndpoint != ''
  ? oidcAuthEndpoint
  : issuerBaseForDerivedEndpoints != ''
    ? '${issuerBaseForDerivedEndpoints}/protocol/openid-connect/auth'
    : ''
var effectiveOidcTokenEndpoint = oidcTokenEndpoint != ''
  ? oidcTokenEndpoint
  : issuerBaseForDerivedEndpoints != ''
    ? '${issuerBaseForDerivedEndpoints}/protocol/openid-connect/token'
    : ''
var effectiveOidcUserInfoEndpoint = oidcUserInfoEndpoint != ''
  ? oidcUserInfoEndpoint
  : issuerBaseForDerivedEndpoints != ''
    ? '${issuerBaseForDerivedEndpoints}/protocol/openid-connect/userinfo'
    : ''

var isLocalAi = aiProvider == 'local'
var hasAi = aiProvider != ''
var guardrailsActive = enableAgentGuardrails && hasAi

// ---------------------------------------------------------------------------
// Agent guardrails — deterministic coordinates.
//
// These names MUST match what the kubernetes-agt-sidecar recipe computes from
// `context.resource.name` (see recipes/agent-guardrails/kubernetes-agt-sidecar.bicep).
// Computing them here, instead of reading the recipe outputs via
// `tradingAgentGuardrails.properties.*`, avoids the conditional dependency
// edge that crashes the Radius Deployment Engine in v0.57.x with
// "Unable to fetch resource reference from callback DeploymentResourceNoOperationJob".
// ---------------------------------------------------------------------------
var agentGuardrailsResourceName  = 'ai-agent-guardrails'
var agentGuardrailsConfigMapName = '${toLower(replace(agentGuardrailsResourceName, '_', '-'))}-policies'
var agentGuardrailsProxyPort     = 8081
var agentGuardrailsMetricsPort   = 9091
var agentGuardrailsMountPath     = '/policies'
var agentGuardrailsDefaultImage  = 'ghcr.io/microsoft/agentmesh/governance-sidecar:4.0.0'
var agentGuardrailsEffectiveImage = agentGuardrailsImage != '' ? agentGuardrailsImage : agentGuardrailsDefaultImage

// Pod patch injected into both ai-agent variants when guardrails are active.
// Empty otherwise — Radius treats `pod: {}` as a no-op merge.
var agentGuardrailsPodPatch = guardrailsActive ? {
  containers: [
    {
      name: 'agt-sidecar'
      image: agentGuardrailsEffectiveImage
      imagePullPolicy: 'IfNotPresent'
      ports: [
        {
          name: 'agt-proxy'
          containerPort: agentGuardrailsProxyPort
          protocol: 'TCP'
        }
        {
          name: 'agt-metrics'
          containerPort: agentGuardrailsMetricsPort
          protocol: 'TCP'
        }
      ]
      env: [
        { name: 'HOST',        value: '0.0.0.0' }
        { name: 'PORT',        value: '${agentGuardrailsProxyPort}' }
        { name: 'POLICY_DIR',  value: agentGuardrailsMountPath }
        { name: 'LOG_LEVEL',   value: 'INFO' }
        { name: 'AGT_MODE',    value: agentGuardrailsMode }
      ]
      volumeMounts: [
        {
          name: 'agt-policies'
          mountPath: agentGuardrailsMountPath
          readOnly: true
        }
      ]
      readinessProbe: {
        httpGet: {
          path: '/ready'
          port: agentGuardrailsProxyPort
        }
        initialDelaySeconds: 5
        periodSeconds: 10
      }
      livenessProbe: {
        httpGet: {
          path: '/health'
          port: agentGuardrailsProxyPort
        }
        initialDelaySeconds: 15
        periodSeconds: 20
      }
      resources: {
        requests: {
          cpu: '100m'
          memory: '256Mi'
        }
        limits: {
          cpu: '500m'
          memory: '512Mi'
        }
      }
    }
  ]
  volumes: [
    {
      name: 'agt-policies'
      configMap: {
        name: agentGuardrailsConfigMapName
      }
    }
  ]
} : {}

// Env vars surfaced into the ai-agent application so it can call the sidecar.
// The agent code is responsible for actually invoking the governance API
// before executing tools / forwarding prompts (per upstream AGT roadmap,
// transparent interception is not yet available).
var agentGuardrailsAgentEnv = guardrailsActive ? {
  GOVERNANCE_PROXY:   { value: 'http://localhost:${agentGuardrailsProxyPort}' }
  GOVERNANCE_API:     { value: 'http://localhost:${agentGuardrailsProxyPort}' }
  GOVERNANCE_ENABLED: { value: 'true' }
  GOVERNANCE_MODE:    { value: agentGuardrailsMode }
} : {}

var kubernetesMetadataExtension = enableIstioInjection ? [
  {
    kind: 'kubernetesMetadata'
    annotations: {
      'sidecar.istio.io/inject': 'true'
    }
    labels: {
      'azure.workload.identity/use': 'true'
    }
  }
] : [
  {
    kind: 'kubernetesMetadata'
    labels: {
      'azure.workload.identity/use': 'true'
    }
  }
]

resource tradingApp 'Applications.Core/applications@2023-10-01-preview' = {
  name: 'adaptive-apps'
  properties: {
    environment: environment
  }
}

resource tradingDb 'Radius.Resources/postgreSqlDatabases@2025-08-01-preview' = {
  name: 'trading-db'
  properties: {
    environment: environment
    application: tradingApp.id
    size: 'S'
  }
}

resource tradingMqtt 'Radius.Resources/mqttBrokers@2025-08-01-preview' = {
  name: 'trading-mqtt'
  properties: {
    environment: environment
    application: tradingApp.id
  }
}

resource backendIdentity 'Radius.Resources/workloadIdentities@2025-08-01-preview' = {
  name: 'backend-identity'
  properties: {
    environment: environment
    application: tradingApp.id
    #disable-next-line BCP073
    clientId: backendClientId
    serviceAccountName: workloadIdentityServiceAccountName
  }
}

resource frontendIdentity 'Radius.Resources/workloadIdentities@2025-08-01-preview' = {
  name: 'frontend-identity'
  properties: {
    environment: environment
    application: tradingApp.id
    #disable-next-line BCP073
    clientId: frontendClientId
    serviceAccountName: workloadIdentityServiceAccountName
  }
}

// ---------------------------------------------------------------------------
// Governance (policy decision point) — opt-in.
//
// When enableGovernance=true the registered recipe (default: OPA) deploys a
// PDP into the app namespace. Containers don't take a connection to this
// resource (governance is applied at the mesh / sidecar layer, not via env
// vars), so there is no `connections` wiring on the containers below.
// ---------------------------------------------------------------------------
resource tradingGovernance 'Radius.Resources/governance@2025-08-01-preview' = if (enableGovernance) {
  name: 'trading-governance'
  properties: {
    environment: environment
    application: tradingApp.id
    mode: governanceMode
    istioIntegration: governanceIstioIntegration
  }
}

// ---------------------------------------------------------------------------
// Agent guardrails (in-pod AGT sidecar) — opt-in, AI-only.
//
// The recipe only stages the policies ConfigMap. The sidecar container
// itself is injected into the ai-agent pod below via `runtimes.kubernetes.pod`
// using the DETERMINISTIC NAMES defined in the `agentGuardrails*` vars above
// — the containers must not read `tradingAgentGuardrails.properties.*` or
// they would create a conditional dependency edge (see header comment).
// ---------------------------------------------------------------------------
resource tradingAgentGuardrails 'Radius.Resources/agentGuardrails@2025-08-01-preview' = if (guardrailsActive) {
  name: agentGuardrailsResourceName
  properties: {
    environment: environment
    application: tradingApp.id
    mode: agentGuardrailsMode
    policies: agentGuardrailsPolicies
    image: agentGuardrailsImage
  }
}

resource tradingAI 'Radius.Resources/aiModels@2025-08-01-preview' = if (isLocalAi) {
  name: 'trading-ai'
  properties: {
    environment: environment
    application: tradingApp.id
    model: aiModel
  }
}

// When using a Recipe-provisioned AI model, Radius auto-injects
// CONNECTION_AI_PROVIDER, CONNECTION_AI_ENDPOINT, and CONNECTION_AI_MODEL from
// the aiModels connection. Only the secret requires explicit wiring.
// Otherwise, the LLM connection values are supplied as parameters directly.
var aiAgentBaseEnv = {
  ASPNETCORE_URLS: { value: 'http://+:7000' }
  OTEL_SERVICE_NAME: { value: 'trading-ai-agent' }
  OTEL_RESOURCE_ATTRIBUTES: { value: 'service.namespace=adaptive-apps,service.version=1.0.0,deployment.environment=radius' }
  OTEL_EXPORTER_OTLP_ENDPOINT: { value: otelCollectorEndpoint }
  OTEL_EXPORTER_OTLP_PROTOCOL: { value: 'http/protobuf' }
}

// Two separate ai-agent declarations, each guarded by an `if`. This avoids
// a non-conditional resource depending on the conditional `tradingAI`
// resource (which trips the Radius Deployment Engine with
// "Unable to fetch resource reference from callback
// DeploymentResourceNoOperationJob" when isLocalAi=false in v0.57.x).

resource aiAgentLocal 'Applications.Core/containers@2023-10-01-preview' = if (isLocalAi) {
  name: 'ai-agent'
  properties: {
    application: tradingApp.id
    container: {
      image: '${imageRegistry}/ai-agent:${imageTag}'
      ports: {
        http: {
          containerPort: 7000
        }
      }
      env: union(aiAgentBaseEnv, union(agentGuardrailsAgentEnv, {
        CONNECTION_AI_SECRETS_APIKEY: { value: tradingAI!.properties.secrets.apiKey }
      }))
    }
    connections: {
      ai: { source: tradingAI!.id }
    }
    runtimes: {
      kubernetes: {
        pod: agentGuardrailsPodPatch
      }
    }
  }
}

resource aiAgentExternal 'Applications.Core/containers@2023-10-01-preview' = if (hasAi && !isLocalAi) {
  name: 'ai-agent'
  properties: {
    application: tradingApp.id
    container: {
      image: '${imageRegistry}/ai-agent:${imageTag}'
      ports: {
        http: {
          containerPort: 7000
        }
      }
      env: union(aiAgentBaseEnv, union(agentGuardrailsAgentEnv, {
        CONNECTION_AI_PROVIDER:       { value: aiProvider }
        CONNECTION_AI_ENDPOINT:       { value: aiEndpoint }
        CONNECTION_AI_MODEL:          { value: aiModelName }
        CONNECTION_AI_SECRETS_APIKEY: { value: aiApiKey }
      }))
    }
    runtimes: {
      kubernetes: {
        pod: agentGuardrailsPodPatch
      }
    }
  }
}

resource backend 'Applications.Core/containers@2023-10-01-preview' = {
  name: 'backend'
  properties: {
    application: tradingApp.id
    container: {
      image: '${imageRegistry}/backend:${imageTag}'
      ports: {
        http: {
          containerPort: 8080
        }
      }
      env: {
        ASPNETCORE_URLS: { value: 'http://+:8080' }
        OTEL_SERVICE_NAME: { value: 'trading-backend' }
        OTEL_RESOURCE_ATTRIBUTES: { value: 'service.namespace=adaptive-apps,service.version=1.0.0,deployment.environment=radius' }
        OTEL_EXPORTER_OTLP_ENDPOINT: { value: otelCollectorEndpoint }
        OTEL_EXPORTER_OTLP_PROTOCOL: { value: 'http/protobuf' }
        // Radius auto-injects CONNECTION_DB_HOST, CONNECTION_DB_PORT,
        // CONNECTION_DB_DATABASE, CONNECTION_DB_USERNAME from the db connection,
        // and CONNECTION_MQTT_HOST, CONNECTION_MQTT_PORT from the mqtt connection.
        // Only secrets and non-connection values require explicit wiring.
        CONNECTION_DB_SECRETS_PASSWORD: { value: tradingDb.properties.secrets.password }
        AZURE_CLIENT_ID: { value: backendIdentity.properties.clientId }
        AZURE_TENANT_ID: { value: workloadIdentityTenantId }
        MQTT_AUTH_METHOD: { value: backendIdentity.properties.authMethod }
        MQTT_TOKEN_AUDIENCE: { value: backendIdentity.properties.tokenAudience }
        MQTT_TOPIC: { value: 'orders/new' }
      }
    }
    extensions: kubernetesMetadataExtension
    connections: {
      db:   { source: tradingDb.id }
      mqtt: { source: tradingMqtt.id }
      identity: { source: backendIdentity.id }
    }
  }
}

resource frontend 'Applications.Core/containers@2023-10-01-preview' = {
  name: 'frontend'
  properties: {
    application: tradingApp.id
    container: {
      image: '${imageRegistry}/frontend:${imageTag}'
      ports: {
        http: {
          containerPort: 3000
        }
      }
      env: {
        PORT:           { value: '3000' }
        OTEL_SERVICE_NAME: { value: 'trading-frontend' }
        OTEL_RESOURCE_ATTRIBUTES: { value: 'service.namespace=portable-apps,service.version=1.0.0,deployment.environment=radius' }
        OTEL_EXPORTER_OTLP_ENDPOINT: { value: otelCollectorEndpoint }
        OTEL_EXPORTER_OTLP_PROTOCOL: { value: 'http/protobuf' }
        // In-cluster service URLs — the frontend server proxies all
        // browser traffic to these endpoints, so only port 3000 is exposed.
        BACKEND_URL:    { value: 'http://backend:8080' }
        AI_AGENT_URL:   { value: 'http://ai-agent:7000' }
        MQTT_WS_URL:    { value: '${tradingMqtt.properties.wsPort == 443 ? 'wss' : 'ws'}://${tradingMqtt.properties.host}:${tradingMqtt.properties.wsPort}' }
        AZURE_CLIENT_ID: { value: frontendIdentity.properties.clientId }
        AZURE_TENANT_ID: { value: workloadIdentityTenantId }
        MQTT_AUTH_METHOD: { value: frontendIdentity.properties.authMethod }
        MQTT_TOKEN_AUDIENCE: { value: frontendIdentity.properties.tokenAudience }
        // OIDC values provided as parameters (from helm-deployed portfolio or external provider).
        OIDC_ISSUER:    { value: effectiveOidcIssuer }
        OIDC_AUTH_ENDPOINT: { value: effectiveOidcAuthEndpoint }
        OIDC_BROWSER_AUTH_ENDPOINT: { value: oidcBrowserAuthEndpoint }
        OIDC_TOKEN_ENDPOINT: { value: effectiveOidcTokenEndpoint }
        OIDC_USERINFO_ENDPOINT: { value: effectiveOidcUserInfoEndpoint }
        OIDC_CLIENT_ID: { value: oidcClientId }
        OIDC_CLIENT_SECRET: { value: oidcClientSecret }
        APP_BASE_URL: { value: appBaseUrl }
        AUTH_USERNAME:  { value: authUsername }
        AUTH_PASSWORD:  { value: authPassword }
        SESSION_SECRET: { value: sessionSecret }
      }
    }
    extensions: kubernetesMetadataExtension
    connections: {
      backend:  { source: backend.id }
      // Note: no Radius `connections` entry for ai-agent. The frontend reaches
      // it via the hardcoded `AI_AGENT_URL` env var above. We deliberately
      // avoid a conditional dependency here because `ai-agent` is declared as
      // two `if`-guarded resources (local vs external AI), and a non-conditional
      // dependency on either trips the Deployment Engine with
      // "Unable to fetch resource reference from callback
      // DeploymentResourceNoOperationJob" in Radius v0.57.x.
      mqtt:     { source: tradingMqtt.id }
      identity: { source: frontendIdentity.id }
    }
  }
}
