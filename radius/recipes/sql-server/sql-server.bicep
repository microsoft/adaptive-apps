// recipes/azure/sql-server.bicep
// Radius Recipe for Radius.Resources/sqlDatabases (Azure SQL Server)
//
// Provisions: Azure SQL Server + single database via AVM.
// Reads:      context.resource.properties.size  → maps to SKU tier
// Outputs:    host, port (1433), database name, admin password

@description('Injected by Radius. Contains resource identity, environment scope, and developer input properties.')
param context object

@description('Database name. Defaults to the Radius resource name.')
param databaseName string = context.resource.name

// ── Size → SKU mapping ──────────────────────────────────────────────────────
var skuMap = {
  S: { name: 'GP_Gen5', tier: 'GeneralPurpose', capacity: 2 }
  M: { name: 'GP_Gen5', tier: 'GeneralPurpose', capacity: 4 }
  L: { name: 'GP_Gen5', tier: 'GeneralPurpose', capacity: 8 }
}
var sizeKey   = context.resource.properties.?size ?? 'S'
var sku       = skuMap[sizeKey]

// ── Derive names and location from context ──────────────────────────────────
var seed       = uniqueString(context.resource.id)
var serverName = 'sql-${take(seed, 10)}'

// The Azure provider scope on the environment gives us subscription + RG.
// Format: /subscriptions/<sub>/resourceGroups/<rg>
var location   = resourceGroup().location

// ── Admin password — generated deterministically from resource ID ────────────
var adminPassword = '${uniqueString(context.resource.id)}Aa1!'

// ── AVM: SQL Server ─────────────────────────────────────────────────────────
// https://aka.ms/avm — search "avm/res/sql/server"
module sqlServer 'br/public:avm/res/sql/server:0.12.0' = {
  name: 'sql-server-${seed}'
  params: {
    name:                    serverName
    location:                location
    administratorLogin:      'sqladmin'
    administratorLoginPassword: adminPassword
    databases: [
      {
        name: databaseName
        sku: {
          name: '${sku.name}_${sku.capacity}'
        }
      }
    ]
    // Security defaults provided by AVM:
    //   - TLS 1.2 minimum
    //   - Public network access can be toggled; leave default for hack
    //   - Auditing, threat detection enabled by default in AVM
    tags: {
      'radapp.io/environment': context.environment.id
      'radapp.io/resource':    context.resource.id
      'radapp.io/application': context.application == null ? '' : context.application.name
    }
  }
}

// ── Result output (required by Radius) ──────────────────────────────────────
output result object = {
  resources: [
    sqlServer.outputs.resourceId
  ]
  values: {
    host:     sqlServer.outputs.fullyQualifiedDomainName
    port:     1433
    database: databaseName
    username: 'sqladmin'
  }
  secrets: {
    password: adminPassword
  }
}