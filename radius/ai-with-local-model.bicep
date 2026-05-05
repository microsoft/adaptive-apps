// ai-with-local-model.bicep — Radius application model for portable-apps with in-cluster AI
//
// Deploys the full stock-trading simulator on Kubernetes via Radius:
//   • Radius.Resources/postgreSqlDatabases  → PostgreSQL 16 (with trading schema)
//   • Radius.Resources/mqttBrokers          → Eclipse Mosquitto 2 (MQTT + WS)
//   • Radius.Resources/idProviders          → OIDC identity provider (Keycloak by default)
//   • Radius.Resources/aiModels             → AI inference endpoint provisioned by a Recipe
//   • Applications.Core/containers          → backend (.NET 8), ai-agent (.NET 8), frontend (Node)
//
// The ai-agent container's LLM connection values (provider, endpoint, model, apiKey) are
// auto-injected by Radius from the aiModels resource connection. Use app.bicep instead
// when you want to supply LLM settings directly as parameters (no Recipe required).
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

@description('Container image registry prefix, e.g. "ghcr.io/myorg" or "myregistry.azurecr.io/portable-apps".')
param imageRegistry string = 'ghcr.io/haishi2016/portable-apps'

@description('Container image tag for all three application services.')
param imageTag string = 'latest'

@description('Username for the local (password-based) frontend login.')
param authUsername string = 'admin'

@description('Password for the local frontend login.')
@secure()
param authPassword string = 'admin'

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

@description('Optional OIDC authorization endpoint override. When empty, derived from oidcIssuer.')
param oidcAuthEndpoint string = ''

@description('Optional OIDC token endpoint override. When empty, derived from oidcIssuer.')
param oidcTokenEndpoint string = ''

@description('Optional OIDC user info endpoint override. When empty, derived from oidcIssuer.')
param oidcUserInfoEndpoint string = ''

@description('OIDC issuer URL (e.g., https://keycloak.example.com/realms/master). Used if oidcIssuerOverride is not set.')
param oidcIssuer string = ''

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

// No browser-facing URL parameters needed — the frontend server
// proxies all backend, AI-agent, and MQTT traffic. Only the frontend
// port needs to be exposed.

@description('''
Model or deployment name hint for the AI Recipe.
  Kaito (kubernetes-kaito.bicep)       : a Kaito preset, e.g. llama-3.1-8b-instruct
  Azure OpenAI (azure-openai.bicep)    : a model name to deploy, e.g. gpt-4o
The Recipe uses this value when provisioning the inference backend.
''')
param aiModel string = 'Qwen/Qwen3-0.6B'

var otelCollectorConfig = '''
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318

processors:
  batch:

exporters:
  zipkin:
    endpoint: http://zipkin:9411/api/v2/spans
  prometheusremotewrite:
    endpoint: http://prometheus:9090/api/v1/write
    tls:
      insecure: true

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [batch]
      exporters: [zipkin]
    metrics:
      receivers: [otlp]
      processors: [batch]
      exporters: [prometheusremotewrite]
'''

resource tradingApp 'Applications.Core/applications@2023-10-01-preview' = {
  name: 'portable-apps'
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

resource tradingAI 'Radius.Resources/aiModels@2025-08-01-preview' = {
  name: 'trading-ai'
  properties: {
    environment: environment
    application: tradingApp.id
    model: aiModel
  }
}

resource zipkin 'Applications.Core/containers@2023-10-01-preview' = {
  name: 'zipkin'
  properties: {
    application: tradingApp.id
    container: {
      image: 'openzipkin/zipkin:3.5.1'
      ports: {
        http: {
          containerPort: 9411
        }
      }
    }
  }
}

resource prometheus 'Applications.Core/containers@2023-10-01-preview' = {
  name: 'prometheus'
  properties: {
    application: tradingApp.id
    container: {
      image: 'prom/prometheus:v2.54.1'
      command: [
        '/bin/prometheus'
        '--config.file=/etc/prometheus/prometheus.yml'
        '--web.enable-remote-write-receiver'
      ]
      ports: {
        http: {
          containerPort: 9090
        }
      }
    }
  }
}

resource otelCollector 'Applications.Core/containers@2023-10-01-preview' = {
  name: 'otel-collector'
  properties: {
    application: tradingApp.id
    container: {
      image: 'otel/opentelemetry-collector-contrib:0.111.0'
      command: [
        '/otelcol-contrib'
        '--config=env:OTEL_COLLECTOR_CONFIG'
      ]
      env: {
        OTEL_COLLECTOR_CONFIG: { value: otelCollectorConfig }
      }
      ports: {
        'otlp-http': {
          containerPort: 4318
        }
      }
    }
    connections: {
      zipkin: { source: zipkin.id }
      prometheus: { source: prometheus.id }
    }
  }
}

resource aiAgent 'Applications.Core/containers@2023-10-01-preview' = {
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
      env: {
        ASPNETCORE_URLS: { value: 'http://+:7000' }
        OTEL_SERVICE_NAME: { value: 'trading-ai-agent' }
        OTEL_RESOURCE_ATTRIBUTES: { value: 'service.namespace=portable-apps,service.version=1.0.0,deployment.environment=radius' }
        OTEL_EXPORTER_OTLP_ENDPOINT: { value: 'http://otel-collector:4318' }
        OTEL_EXPORTER_OTLP_PROTOCOL: { value: 'http/protobuf' }
        // Radius auto-injects CONNECTION_AI_PROVIDER, CONNECTION_AI_ENDPOINT,
        // and CONNECTION_AI_MODEL from the aiModels connection below.
        // Only the secret requires explicit wiring (secrets are not auto-injected).
        CONNECTION_AI_SECRETS_APIKEY: { value: tradingAI.properties.secrets.apiKey }
      }
    }
    connections: {
      ai: { source: tradingAI.id }
      otel: { source: otelCollector.id }
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
        OTEL_RESOURCE_ATTRIBUTES: { value: 'service.namespace=portable-apps,service.version=1.0.0,deployment.environment=radius' }
        OTEL_EXPORTER_OTLP_ENDPOINT: { value: 'http://otel-collector:4318' }
        OTEL_EXPORTER_OTLP_PROTOCOL: { value: 'http/protobuf' }
        // Radius auto-injects CONNECTION_DB_HOST, CONNECTION_DB_PORT,
        // CONNECTION_DB_DATABASE, CONNECTION_DB_USERNAME from the db connection,
        // and CONNECTION_MQTT_HOST, CONNECTION_MQTT_PORT from the mqtt connection.
        // Only secrets and non-connection values require explicit wiring.
        CONNECTION_DB_SECRETS_PASSWORD: { value: tradingDb.properties.secrets.password }
        MQTT_TOPIC: { value: 'orders/new' }
      }
    }
    connections: {
      db:   { source: tradingDb.id }
      mqtt: { source: tradingMqtt.id }
      otel: { source: otelCollector.id }
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
        OTEL_EXPORTER_OTLP_ENDPOINT: { value: 'http://otel-collector:4318' }
        OTEL_EXPORTER_OTLP_PROTOCOL: { value: 'http/protobuf' }
        // In-cluster service URLs — the frontend server proxies all
        // browser traffic to these endpoints, so only port 3000 is exposed.
        BACKEND_URL:    { value: 'http://backend:8080' }
        AI_AGENT_URL:   { value: 'http://ai-agent:7000' }
        MQTT_WS_URL:    { value: 'ws://${tradingMqtt.properties.host}:${tradingMqtt.properties.wsPort}' }
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
    connections: {
      backend: { source: backend.id }
      aiModel: { source: tradingAI.id }
      mqtt:    { source: tradingMqtt.id }
      otel:    { source: otelCollector.id }
    }
  }
}
