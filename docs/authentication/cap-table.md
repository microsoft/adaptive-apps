
# Authentication Scenarios

The following table summarizes common authentication-related scenarios and how Adaptive Apps handles them. In each case, **business continuity is sustained**, though some capabilities may be degraded.


| Capability | Normal | Entra Outage | Azure/Internet Outage | Break-glass| Airgapped |
|--------|--------|--------|--------|-----|--------|
| [Federation with local IdP](keycloak-active-directory.md) | ✅Yes | ✅Yes | ✅Yes | ✅Yes | ✅Yes |
| [Local AD credential sync to Entra](microsoft-entra-connect.md) | ✅Yes | ❌No | ❌No | ➖N/A | ➖N/A |
| Service-to-Service authentication with mTLS | ✅Yes | ✅Yes | ✅Yes | ✅Yes | ✅Yes |
| User authentication with Azure Entra | ✅Yes | 💾Cached<sup>1</sup> | 💾Cached<sup>1</sup> | KeyCloak<sup>2</sup> | KeyCloak<sup>2</sup> |
| User/group/policy management with Azure Entra | ✅Yes | ✅Yes | ✅Yes | ✅Yes | ❌No<sup>3</sup> | 
| Workload identiy to authenticate with Azure services | ✅Yes | 💾Cached | 💾Cached | ✅Yes/Cached | ❌No |

1. Existing security tokens can be cached up to the lifetime allowed by policy, usually 72 hours. New logins are not possible.
2. Keycloak is used as an independent IdP with locally managed credentials, or is configured to federate with a local IdP.
3. Managed by Keycloak or a local IdP.