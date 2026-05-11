# The Min Portfolio

The Min Portfolio is designed for local development, prototyping, and MVP scenarios. It includes the minimal set of components recommended by Adaptive Apps to enable faster deployment and simplified configuration. This portfolio enables:

* Continued access to the application even when disconnected from the internet, enabled by a local identity provider.

## Components

* [KeyCloak](https://www.keycloak.org/) as the identity provider for user authentication.
* [Workload Identities](https://learn.microsoft.com/en-us/entra/workload-id/workload-identities-overview) for authenticating with Azure services.

## Authentication

Adaptive Apps recommends a claims-based architecture for authentication and authorization. In this model, applications delegate authentication to a trusted identity provider (IdP), which authenticates users and issues security tokens containing claims that describe user attributes, such as identity and roles. Applications validate the token, extract the claims, and use them to enforce authorization policies.

The use of Keycloak enables continued user authentication even in disconnected scenarios (e.g., without internet or cloud connectivity). While Keycloak can function as a standalone identity provider, it is typically configured to federate with an upstream identity provider, such as an on-premises Active Directory. Refer to the additional guidance below for more details.

### Service-to-Service authentication

The Min Portfolio does not include service identities; consequently, user tokens are used for access control to backend resources. In this model, backend services are either accessible within the cluster boundary—loosely following a trusted subsystem pattern—or configured to trust the same identity provider (IdP) as the frontend for token validation.

The [Core Portfoilo](./core.md) formally introduces service identities, providing stronger security boundaries, clearer separation of concerns, and a more production-ready approach to service-to-service authentication and authorization.

### Integration with Azure Entra ID

Azure Entra ID can be integrated in two primary ways. First, it can be configured as a federated identity provider (IdP) with Keycloak, allowing Keycloak to broker authentication requests to Entra ID. Second, since Azure Entra ID supports the OIDC standard, frontend applications can be configured to authenticate directly against Entra ID, bypassing Keycloak entirely.

The first approach provides greater flexibility, as Keycloak can be configured with multiple upstream IdPs and act as a centralized identity broker. This is particularly useful in hybrid or disconnected scenarios, where Keycloak can continue to authenticate users through locally available identity sources (for example, LDAP or cached credentials), even when Entra ID is not reachable. See [this doc](../authentication/keycloak-entra.md) for more details.

## Additional Guidance

* [How to deploy KeyCloak behind an ingress](../authentication/keycloak-ingress.md)
* [How to set up KeyCloak federation with Azure Entra ID](../authentication/keycloak-entra.md)
* [How to set up KeyCloak federation with a local Active Directory](../authentication/keycloak-active-directory.md)
* [How to sync local Active Directory credentails to an Azure Entra tenant](../authentication/microsoft-entra-connect.md)