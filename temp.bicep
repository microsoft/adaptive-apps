I will create an application definition for `src`.
First, let me review the platform constitution.
> Read cloud-native application architecture patterns.
> Read available resource types.
> Read resource types composition rules.
> Read resource types dependencies.
> Read naming conventions.
> Read secret handling requirements.
I see this repository has a single application called `src`.
I will identify what abstract application resources this application uses.
It has these application resources:
1. MQTT Broker: src-mqtt
2. PostgreSQL Database: src-postgres
An application definition has been created for `src`.

```bicep
extension radius
extension radiusCompute
extension radiusSecurity
extension radiusData

param environment string

@secure() param password string
@description('The full container image reference to build and push. Must be lowercase.')
param image string

resource srcApp 'Applications.Core/applications@2023-10-01-preview' = {
  name: 'src'
  properties: {
    environment: environment
  }
}

resource srcPostgres 'Radius.Data/postgreSqlDatabases@2025-08-01-preview' = {
  name: 'src-postgres'
  properties: {
    environment: environment
    application: srcApp.id
    size: 'S'
    secretName: dbSecret.name
  }
}

resource dbSecret 'Radius.Security/secrets@2025-08-01-preview' = {
  name: 'dbsecret'
  properties: {
    environment: environment
    application: srcApp.id
    data: {
      USERNAME: {
        value: 'trade'
      }
      PASSWORD: {
        value: password
      }
    }
  }
}

resource srcMqtt 'Radius.Resources/mqttBrokers@2025-08-01-preview' = {
  name: 'src-mqtt'
  properties: {
    environment: environment
    application: srcApp.id
  }
}

resource aiAgent 'Radius.Compute/containers@2025-08-01-preview' = {
  name: 'aiAgent'
  properties: {
    environment: environment
    application: srcApp.id
    containers: {
      aiAgent: {
        image: image
        ports: {
          http: {
            containerPort: 7000
          }
        }
        env: {
          ASPNETCORE_URLS: {
            value: 'http://+:7000'
          }
          CONNECTION_AI_ENDPOINT: {
            value: ''
          }
          CONNECTION_AI_MODEL: {
            value: 'gpt-4o'
          }
          CONNECTION_AI_PROVIDER: {
            value: 'openai'
          }
          CONNECTION_AI_SECRETS_APIKEY: {
            value: ''
          }
        }
      }
    }
    connections: {
      mqtt: {
        source: srcMqtt.id
      }
    }
  }
}

resource backend 'Radius.Compute/containers@2025-08-01-preview' = {
  name: 'backend'
  properties: {
    environment: environment
    application: srcApp.id
    containers: {
      backend: {
        image: image
        ports: {
          http: {
            containerPort: 8080
          }
        }
        env: {
          ASPNETCORE_URLS: {
            value: 'http://+:8080'
          }
          CONNECTION_DB_DATABASE: {
            value: 'trading'
          }
          CONNECTION_DB_HOST: {
            value: 'src-postgres'
          }
          CONNECTION_DB_PORT: {
            value: '5432'
          }
          CONNECTION_DB_SECRETS_PASSWORD: {
            value: password
          }
          CONNECTION_DB_USERNAME: {
            value: 'trade'
          }
          CONNECTION_MQTT_HOST: {
            value: 'src-mqtt'
          }
          CONNECTION_MQTT_PORT: {
            value: '1883'
          }
          MQTT_TOPIC: {
            value: 'orders/new'
          }
        }
      }
    }
    connections: {
      mqtt: {
        source: srcMqtt.id
      }
      db: {
        source: srcPostgres.id
      }
    }
  }
}

resource frontend 'Radius.Compute/containers@2025-08-01-preview' = {
  name: 'frontend'
  properties: {
    environment: environment
    application: srcApp.id
    containers: {
      frontend: {
        image: image
        ports: {
          http: {
            containerPort: 3000
          }
        }
        env: {
          AAD_CLIENT_ID: {
            value: 'test'
          }
          AAD_CLIENT_SECRET: {
            value: 'test'
          }
          AAD_REDIRECT_URI: {
            value: 'http://localhost:3000/auth/microsoft/callback'
          }
          AAD_TENANT_ID: {
            value: 'common'
          }
          AI_AGENT_URL: {
            value: 'http://ai-agent:7000'
          }
          AUTH_PASSWORD: {
            value: 'admin'
          }
          AUTH_USERNAME: {
            value: 'admin'
          }
          BACKEND_URL: {
            value: 'http://backend:8080'
          }
          MQTT_WS_URL: {
            value: 'ws://src-mqtt:9001'
          }
          PORT: {
            value: '3000'
          }
          SESSION_SECRET: {
            value: 'change-me-in-production'
          }
        }
      }
    }
    connections: {
      mqtt: {
        source: srcMqtt.id
      }
    }
  }
}
```
Would you like me to create a pull request with this file?