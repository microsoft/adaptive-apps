# The Min Portfolio

The Min Portfolio is designed for local development, prototyping, and MVP scenarios. It includes the minimal set of components recommended by Adaptive Apps to enable faster deployment and simplified configuration. This portfolio enables:

* Continued access to the application even when disconnected from the internet, enabled by a local identity provider.

## Components

* [KeyCloak](https://www.keycloak.org/) as the identity provider for authentication.


## Authentication

Adaptive Apps recommends a claims-based architecture for authentication and authorization. In this model, applications delegate authentication to a trusted identity provider (IdP), which authenticates users and issues security tokens containing claims that describe user attributes, such as identity and roles. Applications validate the token, extract the claims, and use them to enforce authorization policies.

The Min Portfolio does not include service identities, and user tokens are therefore used for access control to backend resources. This loosely aligns with a trusted subsystem pattern, where backend services operate on behalf of the user. The [Core Portfoilo](./core.md) formally introduces service identities, providing stronger security boundaries, clearer separation of concerns, and a more production-ready approach to service-to-service authentication and authorization.

The use of Keycloak enables continued user authentication even in disconnected scenarios (e.g., without internet or cloud connectivity). While Keycloak can function as a standalone identity provider, it is typically configured to federate with an upstream identity provider, such as an on-premises Active Directory. Refer to the additional guidance below for more details.


## Additional Guidance

* [How to deploy KeyCloak behind an ingress](../authentication/keycloak-ingress.md)
* [How to set up KeyCloak federation with a local Active Directory](../authentication/keycloak-active-directory.md)
* [How to sync local Active Directory credentails to an Azure Entra tenant](../authentication/microsoft-entra-connect.md)