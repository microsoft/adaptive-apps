# Trading Advisor memory

Memory is opt-in. `ADVISOR_MEMORY_PROVIDER=none` (the default) preserves the
existing advisor. The advisor remains the .NET `/advice` service; this does not
change the Agents tab, portable-agent artifacts, or native Responses storage.

The PostgreSQL demo uses separate databases and an independent sync process:

```text
AKS advisor -> AKS PostgreSQL -> sync utility -> local PostgreSQL <- local advisor
```

AKS memory is explicitly writable; local memory defaults to read-only.
Azure Table Storage remains an optional cloud source.

With memory enabled, the advisor accepts these explicit commands:

* `Remember: The king has donkey ears!` persists an immutable fact in the
  writable cloud store. Acknowledgment occurs only after a successful insert.
* `Recall memories` or `What did I ask you to remember?` reads up to the 100
  most recent facts without model inference. This makes the demo independent
  of model tool-calling accuracy or conversation history.
* Other advisor questions receive the same user's recent facts as reference
  data. Facts are not system instructions. This is bounded exact-history
  retrieval, not semantic/vector search.

The read-only local PostgreSQL provider rejects `Remember:` with HTTP 409. It
never writes back to the cloud. Storage failures return HTTP 503 rather than pretending
there are no memories. Missing tokens return 401; missing, duplicate, or
unapproved Entra identity claims return 403. Local password login cannot use
memory-enabled advice. The existing `{ "question": ... }` / `{ "answer": ... }`
HTTP contract is unchanged.

## Trust and identity prerequisites

The default identity mode is `entra`. Configure **both** Keycloak installations
as follows for Entra identity; the explicitly gated local-user demo alternative
below does not require Entra federation.

1. In the Entra SAML enterprise application's claims, ensure the signed
   assertion supplies the user's object ID and issuing tenant ID:
   `http://schemas.microsoft.com/identity/claims/objectidentifier` and
   `http://schemas.microsoft.com/identity/claims/tenantid`.
   The object ID must be the same directory object in both deployments.
   Guest objects in different tenants are not interchangeable.
2. In realm `adaptive`, add SAML identity-provider Attribute Importer mappers
   (sync mode **Force**) to import these into user attributes `entra_oid` and
   `entra_tid`. Require valid signed SAML assertions from the reviewed Entra
   tenant and signing certificate. A name/email is not a substitute.
3. Configure the Keycloak user profile so these attributes are **admin-editable
   only**, not editable at registration, first-login profile review, or account
   self-service. Do not copy user-entered attributes into these claims.
4. On client `adaptive-trading` (or its dedicated client scope), add OIDC
   **User Attribute** mappers: user attribute and token claim `entra_oid` and
   `entra_tid`, JSON type String, **Add to access token** enabled.
5. Add an **Audience** mapper to that client: included custom audience
   `trading-advisor`, **Add to access token** enabled. Do not enable this
   audience on unrelated clients.
6. Sign out and sign in through Entra again. Existing sessions/tokens will not
   automatically gain the new mappings. Verify the claims locally; never paste
   bearer tokens into logs, chat, or public JWT-decoding websites.

The advisor validates token signature, expiry, exact local Keycloak issuer,
and audience. It hashes `[source ID, agent ID, Entra tenant ID, Entra object ID]`
as the partition key. Different Keycloak `sub`/issuer values therefore do not
prevent the same verified Entra user recalling the same replicated memories.
Claims are never taken from browser headers or request JSON.

The demo's Entra authentication tenant and AKS infrastructure tenant do **not**
need to match. Set `ADVISOR_MEMORY_ENTRA_TENANT_ID` to the demo user's tenant
on both deployments. PostgreSQL authenticates with separate database roles;
it does not use this user claim or require Azure Storage/workload identity.
The optional Table provider uses the AKS workload identity independently.

### Shared authenticated demo memory

For a demo where memory belongs to the **application**, not an individual
user, explicitly set `ADVISOR_MEMORY_IDENTITY_MODE=shared-demo` on both advisors.
Set the same nonempty UUID in `ADVISOR_MEMORY_SHARED_SCOPE_ID` on both. Radius
parameters are `advisorMemoryIdentityMode=shared-demo` and
`advisorMemorySharedScopeId=<shared-uuid>`.

**All authenticated advisor users can read the same facts, and all can append
facts on the writable AKS deployment. There is no per-user privacy in this
mode. Do not store private user information.** Local memory remains read-only.
Each deployment still validates its own Keycloak issuer, JWT signature,
audience, and expiry. Anonymous requests and the frontend's separate
non-OIDC password login remain disallowed.

Use your existing OIDC login on each system. Entra claims, matching subjects,
matching usernames, and a special interactive demo account are not required.
Database roles used by sync are entirely separate from browser users.
The `trading-advisor` access-token audience is still required; an older session
may need sign-out/sign-in to acquire that audience.

Shared partitions hash `[source ID, agent ID, "shared-demo", shared UUID]`,
separate from Entra and mapped-user demo partitions. Selecting shared mode
does not migrate or delete facts from those other scopes. Re-save demo facts
in the shared scope explicitly. This mode is never enabled automatically or
used as a fallback for failed user-identity verification.

### Optional mapped Keycloak-local demo without Entra access

Set `ADVISOR_MEMORY_IDENTITY_MODE=keycloak-demo` explicitly on both advisors.
Create a dedicated local user in each Keycloak realm and sign in using the
frontend's **OIDC button**, then that Keycloak user's credentials. The frontend's
separate local-password login still cannot access memory.

Configure these server-side values:

| Variable | Value |
|---|---|
| `ADVISOR_MEMORY_DEMO_SUBJECT` | This deployment's exact Keycloak user UUID (`sub`), different in each realm |
| `ADVISOR_MEMORY_DEMO_USER_ID` | One reviewed, nonempty UUID shared across both deployments for this demo only |

Add the `trading-advisor` access-token audience mapper to the trading client
in both realms. Keep issuer, signature, audience, and expiry validation enabled.
Only the configured subject is authorized; matching usernames/emails, browser
headers, Entra claims, or other realm users do not grant demo memory access.
No fake Entra tenant/object-ID attributes are used. Missing/invalid mappings
fail explicitly, and there is no automatic fallback from Entra mode.

Demo partitions hash `[source ID, agent ID, "keycloak-demo", demo UUID]`,
separate from Entra partitions. Changing the demo UUID starts a different
memory scope; do not silently remap users or migrate memories across identities.
This intentionally single-user demo mode is not a production identity-linking
service. Retain the provisioned user IDs and credentials; do not recreate users
or regenerate passwords on repeat setup.

Radius equivalents are `advisorMemoryIdentityMode=keycloak-demo`,
`advisorMemoryDemoSubject=<this-realm-user-uuid>`, and
`advisorMemoryDemoUserId=<shared-demo-uuid>`. Entra mode remains the default
and still requires `advisorMemoryEntraTenantId`.

The frontend forwards its server-held OIDC access token only when
`ADVISOR_MEMORY_ENABLED=true`; the advisor validates it independently.
OIDC discovery and signing-key retrieval can use the internal Keycloak realm
endpoint even when the public issuer uses a workstation port-forward.
Internal HTTP metadata is accepted only for loopback or
`*.svc.cluster.local`; its network and DNS must be trusted. Use HTTPS
metadata for other endpoints. Do not disable TLS certificate verification.

## Provider configuration

Common advisor variables:

| Variable | Value |
|---|---|
| `ADVISOR_MEMORY_PROVIDER` | `postgres` on AKS and locally; optional `table` on AKS; default `none` |
| `ADVISOR_MEMORY_SOURCE_ID` | A reviewed stable source name, e.g. `trading-memory-v1`; identical on both |
| `ADVISOR_MEMORY_AGENT_ID` | `trading-advisor`; identical on both |
| `ADVISOR_MEMORY_ENTRA_TENANT_ID` | The user's Entra tenant, not necessarily the AKS subscription tenant |
| `ADVISOR_MEMORY_OIDC_ISSUER` | This deployment's exact public Keycloak realm URL |
| `ADVISOR_MEMORY_OIDC_AUDIENCE` | `trading-advisor` |
| `ADVISOR_MEMORY_OIDC_METADATA` | This deployment's internal realm discovery URL |

For example, the AKS issuer can be `https://localhost:8445/realms/adaptive`
and the local issuer `https://localhost:8444/realms/adaptive`, while metadata
is `http://min-ai-keycloak.min-ai.svc.cluster.local:8080/realms/adaptive/.well-known/openid-configuration`
within each cluster. Both independently verify their own Keycloak tokens.

PostgreSQL advisor variables:

| Variable | Value |
|---|---|
| `ADVISOR_MEMORY_POSTGRES` | This deployment's private PostgreSQL connection string |
| `ADVISOR_MEMORY_POSTGRES_READ_ONLY` | Default `true`; explicitly `false` on the AKS writer; keep `true` locally |

An invalid read-only setting fails startup. Use a cloud advisor role with
SELECT/INSERT only, a separate source-export role with SELECT only, a local
advisor role with SELECT only, and a local sync role with SELECT/INSERT/UPDATE.
Do not use the database-owner role for these services. Both databases use the
same empty `advisor_memory` schema; applying it is a separate operator action.
Keep connection strings in private environments/Secrets, not source control.
Configure PostgreSQL TLS with server-certificate verification for non-local
connections, and restrict database network access to the advisor and sync host.

Optional Azure Table provider and sync-source variables:

| Variable | Value |
|---|---|
| `ADVISOR_MEMORY_TABLE_ENDPOINT` | `https://<account>.table.core.windows.net` |
| `ADVISOR_MEMORY_TABLE_NAME` | Pre-created table, e.g. `AdvisorMemory` |
| `ADVISOR_MEMORY_CREDENTIAL` | `workload` for AKS; `managed` for an Azure managed-identity host; `developer` for explicit Azure CLI authentication |
| `AZURE_CLIENT_ID` | User-assigned managed identity client ID for `workload`/`managed` |

There is **no credential fallback**. AKS uses `WorkloadIdentityCredential`
with the Azure workload-identity token file and tenant environment injected by
the AKS webhook. `developer` is deliberately opt-in for the sync workstation;
the signed-in identity needs **Storage Table Data Reader** on the source
table. The Table provider does not use storage keys or storage connection strings.

PostgreSQL connection-string shape (advisor and sync use different roles):

```text
ADVISOR_MEMORY_POSTGRES=Host=<host>;Database=trading;Username=<role>;Password=<secret>
```

Keep it in a private environment/Secret, not source control. Configure
appropriate PostgreSQL TLS for non-local connections. The local advisor uses a
**SELECT-only role**, while the sync utility uses its own insert/update role.
Schema application is a separate operator action, never an advisor startup
side effect.

## Optional Azure Table storage and AKS identity

Skip this section for PostgreSQL on AKS. No Table account, managed identity,
or dedicated workload-identity service account is needed for PostgreSQL memory.

Use an approved storage account with shared-key access disabled where possible,
HTTPS enforced, and network access restricted to the advisor and sync host.
Create the `AdvisorMemory` table with an authorized provisioning identity:

```bash
az storage table create --account-name "$MEMORY_ACCOUNT" \
  --name AdvisorMemory --auth-mode login
```

Provision a dedicated advisor managed identity and federate it to service
account `advisor-memory` in the **actual trading application namespace**.
The [existing helper](../../tutorials/getting-started/assets/app-wi-setup.sh)
can provision a new identity; do not rerun creation over retained identities
or regenerate credentials.

```bash
az account set --subscription "$AZURE_SUBSCRIPTION"
./tutorials/getting-started/assets/app-wi-setup.sh advisor-memory \
  "$RESOURCE_GROUP" "$AZURE_SUBSCRIPTION" "$AKS_OIDC_ISSUER" \
  trading-adaptive-apps advisor-memory
```

The helper does not grant Table Storage access. Assign **Storage Table Data
Contributor** on the source table to that managed identity's **principal ID**.
Use **Storage Table Data Reader** for the separate sync identity:

```bash
TABLE_SCOPE="/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP/providers/Microsoft.Storage/storageAccounts/$MEMORY_ACCOUNT/tableServices/default/tables/AdvisorMemory"
az role assignment create --assignee-object-id "$ADVISOR_PRINCIPAL_ID" \
  --assignee-principal-type ServicePrincipal \
  --role "Storage Table Data Contributor" --scope "$TABLE_SCOPE"
```

The service account has separate retained ownership from Radius deployment.
Create it once if absent; if it already exists, inspect its ownership and
identity annotation instead of overwriting it:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: advisor-memory
  namespace: trading-adaptive-apps
  annotations:
    azure.workload.identity/client-id: "<advisor-managed-identity-client-id>"
    azure.workload.identity/tenant-id: "<AKS-managed-identity-tenant-id>"
```

Create with `kubectl --context ada-ignite create -f <reviewed-file>`. The
Radius application model adds the workload-identity Pod label and uses this
service account when the Table memory provider is selected.

## Radius deployment

Build/publish the updated frontend and advisor images using your normal
reviewed image release process. Existing published `latest` images do not
automatically contain this feature. Add parameters to the **complete**
existing `rad deploy radius/app.bicep` command; do not drop its OIDC, AI, or
workload-identity parameters.

The advisor and frontend images run as their base images' non-root users.
Their image health checks use advisor `/health` on port 7000 and frontend
`/auth/providers` on its configured port (3000 by default). Local build outputs,
frontend dependencies, tests, and environment files are excluded from image
build contexts.

AKS PostgreSQL additions:

```text
--parameters advisorMemoryProvider=postgres
--parameters advisorMemoryPostgresReadOnly=false
--parameters advisorMemoryEntraTenantId=<user-tenant-id>
--parameters @<private-aks-memory-parameters.json>
```

Local additions:

```text
--parameters advisorMemoryProvider=postgres
--parameters advisorMemoryPostgresReadOnly=true
--parameters advisorMemoryEntraTenantId=<same-user-tenant-id>
--parameters @<private-memory-parameters.json>
```

Each private file supplies secure parameter `advisorMemoryPostgres` with that
deployment's database connection string: AKS SELECT/INSERT role or local
SELECT-only role. Keep the same source/agent IDs in both deployments; defaults
are `trading-memory-v1` and `trading-advisor`. Radius injects the selected
provider into **both AI deployment variants** (recipe-hosted local model and
external model), enables the frontend token forwarding, derives internal
discovery from the supplied OIDC token endpoint, and preserves each
deployment's public OIDC issuer. This does not change model selection or
route model traffic through a memory/diagnostics service.

For the optional Table source, use `advisorMemoryProvider=table` on AKS,
`advisorMemoryTableEndpoint`, `advisorMemoryClientId`, and
`workloadIdentityTenantId` as described above. PostgreSQL read-only settings
do not change Table behavior. PostgreSQL memory does not select the Table
service account or add its workload-identity metadata extension.

## PostgreSQL schema and synchronization

### Retained database ownership

Inspect the existing trading PostgreSQL Deployment before choosing it for
memory: the sample's database recipe can be ephemeral. A container filesystem
is not durable memory, even if a sync pass succeeded.

The optional [native PostgreSQL manifest](Memory/postgres.yaml) provisions a
separate single-instance memory database, not an operator or a replacement for
trading's database. Before applying it, provision:

* StorageClass `advisor-memory-retained`, using the cluster's reviewed dynamic
  provisioner with **Retain** reclaim policy.
* Separately retained Secret `advisor-memory-database` in
  `trading-adaptive-apps`, with `owner-password`. Generate a password once and
  retain it; never replace credentials for an existing volume.

The StatefulSet uses database `advisor_memory`, owner `memory_owner`, a pinned
PostgreSQL 16 Alpine image, and its native non-root UID/GID 70. Its PVC is
retained on scale-down/deletion. The manifest does not create database roles,
seed memories, enable TLS, or configure network isolation; apply the schema,
dedicated roles, and appropriate network/TLS controls separately. It is a
single-instance demo database, not a highly available production service.

```bash
kubectl --context <reviewed-context> apply -f src/ai-agent/Memory/postgres.yaml
kubectl --context <reviewed-context> -n trading-adaptive-apps \
  rollout status statefulset/advisor-memory-postgres
```

Changing the image or recreating a Pod must not replace the Secret or PVC.
Retain identity credentials separately from this database state.

### Schema and roles

Apply the empty schema to **both** existing PostgreSQL databases, using
separate approved administrative connections:

```bash
psql "$AKS_POSTGRES_ADMIN_URL" -v ON_ERROR_STOP=1 \
  -f src/ai-agent/Memory/schema.sql
psql "$LOCAL_POSTGRES_ADMIN_URL" -v ON_ERROR_STOP=1 \
  -f src/ai-agent/Memory/schema.sql
```

Create/retain dedicated PostgreSQL login roles through your existing secret
management process; do not reuse trading's database-owner role for the advisor.
Grant permissions (role names below are examples). On AKS:

```sql
GRANT USAGE ON SCHEMA advisor_memory TO advisor_memory_writer, advisor_memory_export;
GRANT SELECT, INSERT ON advisor_memory.facts TO advisor_memory_writer;
GRANT SELECT ON advisor_memory.facts TO advisor_memory_export;
```

On the local database:

```sql
GRANT USAGE ON SCHEMA advisor_memory TO advisor_memory_reader, advisor_memory_sync;
GRANT SELECT ON advisor_memory.facts TO advisor_memory_reader;
GRANT SELECT, INSERT, UPDATE ON advisor_memory.facts TO advisor_memory_sync;
GRANT SELECT, INSERT, UPDATE ON advisor_memory.sync_status TO advisor_memory_sync;
```

The `MemorySync` utility is independent of the advisor. Source selection:

| Variable | Value |
|---|---|
| `ADVISOR_MEMORY_SYNC_SOURCE_PROVIDER` | Set `postgres`; defaults to `table` for existing Table sync deployments |
| `ADVISOR_MEMORY_SYNC_SOURCE_POSTGRES` | AKS PostgreSQL connection string using the SELECT-only export role |
| `ADVISOR_MEMORY_POSTGRES` | Local PostgreSQL connection string using the separate sync role |
| `ADVISOR_MEMORY_SOURCE_ID`, `ADVISOR_MEMORY_AGENT_ID` | Identical to both advisor deployments |

PostgreSQL source mode does not read Azure Table endpoint or credential
settings. An unknown source provider or missing source connection fails
explicitly; there is no storage/credential fallback.

If databases are reachable only inside Kubernetes, use separate, explicit
port-forwards for the source and destination, with distinct local ports:

```bash
kubectl --context ada-ignite -n trading-adaptive-apps \
  port-forward --address 127.0.0.1 svc/<aks-postgres-service> 15432:5432
kubectl --context <local-context> -n trading-adaptive-apps \
  port-forward --address 127.0.0.1 svc/<local-postgres-service> 15433:5432
```

Run those in separate terminals; inspect the actual PostgreSQL service names
first. The AKS advisor uses its cluster-internal database address, not the
workstation forward. For a workstation sync run:

```bash
export ADVISOR_MEMORY_SYNC_SOURCE_PROVIDER=postgres
export ADVISOR_MEMORY_SOURCE_ID=trading-memory-v1
export ADVISOR_MEMORY_AGENT_ID=trading-advisor
# Load ADVISOR_MEMORY_SYNC_SOURCE_POSTGRES for the AKS export role and
# ADVISOR_MEMORY_POSTGRES for the local sync role from your private environment.
dotnet run --project src/ai-agent/MemorySync/AdvisorMemorySync.csproj -- --once

# Omit --once for a repeating worker; Ctrl-C stops it.
dotnet run --project src/ai-agent/MemorySync/AdvisorMemorySync.csproj
```

For an existing Table source, explicitly set
`ADVISOR_MEMORY_SYNC_SOURCE_PROVIDER=table`, configure its Table endpoint/name
and credential mode, and retain the local sync connection. For a workstation,
`ADVISOR_MEMORY_CREDENTIAL=developer` requires prior approved Azure CLI login.

The default interval is 30 seconds; set `ADVISOR_MEMORY_SYNC_INTERVAL_SECONDS`
to another positive integer. An existing process supervisor can run the
published console utility in the background. It requires source network
access; the local advisor does not.

Synchronization performs a full scan of this source/agent's
immutable facts. Each insert is idempotent on source, agent, user partition,
and memory ID. Matching rows are accepted; changed content with the same ID
or an unsupported schema fails explicitly. Completed rows from an interrupted
pass remain safe to replay. `sync_status.completed_at` advances only after
the entire pass succeeds; it is **not** a snapshot watermark or proof of
global consistency during concurrent cloud writes.
PostgreSQL exports stream through a database reader; Table exports use SDK
pagination. Exports include **all** scoped facts, not just the 100 recent facts
shown by an advisor. The utility copies only memory facts, not trading data,
users, Keycloak credentials, or model state.

Transient PostgreSQL/network failures and Table request failures are logged
and retried on the next interval in continuous mode. `--once` exits
unsuccessfully on failure; schema/content errors and non-transient PostgreSQL
errors stop the utility. Source and destination credentials are separate.

This version is **append-only**: no edit/delete API, TTL, automatic pruning,
or deletion-by-absence. Physically deleting a cloud fact does not delete a
replica. Add explicit versioned tombstones and a reviewed retention policy
before supporting deletion. Full scans are intended for the demo, not
unbounded production memory. Contents are personal data; configure retention,
consent, and access controls before using real user information.

Inspect sync status with:

```sql
SELECT source_id, agent_id, completed_at, scanned_count FROM advisor_memory.sync_status;
```

## Demonstration and verification

1. In `shared-demo` mode, use your normal OIDC login on each system. In Entra
   mode, use the same verified Entra identity; in `keycloak-demo`, use the
   explicitly mapped realm users.
2. On AKS, send `Remember: The king has donkey ears!`.
3. Run a successful sync pass and inspect `sync_status`.
4. Stop the sync worker or disconnect its cloud network. Keep local Keycloak,
   PostgreSQL, and the local app available.
5. Start a fresh local advisor interaction and send
   `What did I ask you to remember?`.
6. Confirm a local `Remember:` is explicitly rejected. In per-user modes,
   another user cannot recall the fact. In `shared-demo`, another authenticated
   advisor user intentionally can recall it; anonymous access must still fail.

Only for the optional `keycloak-demo` mode, use the OIDC button on each frontend,
then sign in as the dedicated `advisor-demo` realm user. Each cluster retains
its own password in `min-ai/advisor-memory-demo-login`; retrieve it only in
your private terminal:

```bash
kubectl --context <local-or-aks-context> -n min-ai \
  get secret advisor-memory-demo-login -o jsonpath='{.data.password}' |
  base64 --decode
printf '\n'
```

Do not use the frontend's separate password-login form. The mapped-user
`keycloak-demo` mode does not accept unrelated Entra users; `shared-demo` does
accept any user with a valid advisor OIDC token. Sign out of previous sessions and
use separate browser profiles for the two localhost deployments. Use the exact
`Remember:` and `Recall memories` commands above; a model reply such as
"Understood" to a different phrase is not a durable-save acknowledgment.

When hosting sync with user systemd services, keep source/destination database
forwards explicitly pinned to their Kubernetes contexts, store the worker's
environment file with mode 0600, and inspect its completion log. In the
workstation service layout used for this demo:

```bash
systemctl --user status advisor-memory-sync.service
journalctl --user -u advisor-memory-sync.service --no-pager -n 10

# Simulate source-storage disconnection without changing local credentials.
systemctl --user stop advisor-memory-sync.service advisor-memory-postgres@aks.service
# Recall memories locally, then restore synchronization.
systemctl --user start advisor-memory-postgres@aks.service advisor-memory-sync.service
```

The context-pinned `advisor-memory-frontend@local` and
`advisor-memory-frontend@aks` services expose ports 3000 and 3001; do not start
competing forwards on those ports. These workstation services require the
user's session/WSL instance and valid Kubernetes credentials; they are not
cluster-hosted sync or a guarantee of synchronization while the workstation
is off. After a database restart, a request may explicitly return 503 while
its pooled connection is invalidated; retry after the database is Ready.

Recall and remember commands do not call the model. Ordinary trading advice
still requires the configured inference provider; disconnecting from Azure
does not make a cloud model available offline. A fresh Entra sign-in also
requires upstream connectivity, even though an existing local Keycloak
session and the local memory replica need no Azure storage access.

Tests:

```bash
dotnet test src/ai-agent/tests/AdvisorMemory.Tests.csproj
```

The PostgreSQL integration tests are explicitly skipped unless both
`TEST_MEMORY_SOURCE_POSTGRES` and `TEST_MEMORY_POSTGRES` are supplied. Set both
to **separate isolated PostgreSQL databases** with the schema applied; the
source login needs SELECT/INSERT and the destination login SELECT/INSERT/UPDATE.
They verify real advisor HTTP save/recall, immutable cloud inserts, all-fact
export beyond the 100-fact recall limit, repeat synchronization, isolation,
read-only rejection, and failed sync not advancing completion.

The optional Table integration test is explicitly skipped unless both
`TEST_MEMORY_TABLE_CONNECTION` (isolated Azurite connection string) and
`TEST_MEMORY_POSTGRES` (isolated PostgreSQL database with the schema applied)
are supplied. It tests real Table writes, repeat synchronization, isolation,
read-only rejection, conflicting immutable rows, schema validation, and local
recall after removing the source table. No production credentials are used.
