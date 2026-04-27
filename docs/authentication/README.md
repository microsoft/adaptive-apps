# Authentication

Adaptive Apps uses a claims-based architecture for authentication, where an application delegates authentication to a trusted identity provider. The identity provider authenticates users using their credentials and issues a security token to the application. The application then validates the token and extracts the claims it contains. These claims can be used by the application to make authorization decisions.

## User authentication

Adaptive Apps capability profiles use [KeyCloak](https://www.keycloak.org/) as a locally deployable identity provider. In production environments, Keycloak is typically configured with federated identity, integrating with upstream identity providers such as Microsoft Entra ID or Active Directory.

Once an Adaptive App capability profile is deployed, you can acess KeyCloak portal locally by:

1. Port forward to KeyCloak portal

    ```bash
    kubectl port-forward -n min svc/min-keycloak 8080:8080
    ```

2. Open `http://localhost:8080` and sign in with the chart values:

    - username: `admin`
    - password: `admin`

    Then, you can use KeyCloak portal to manage users and credentials.

## Service authentication

Adaptive Apps uses workload identity based on Kubernetes service accounts for service-to-service authentication. Each service runs in a Kubernetes pod associated with a service account. This service account represents the workload’s identity and can be used to obtain a security token from an identity provider.

Two patterns are supported depending on the environment:

### Local / self-managed environments

In local Kubernetes or Arc-enabled clusters, services authenticate using OIDC tokens issued via Keycloak:
1. A service account is mapped to a Keycloak client or identity
2. The workload obtains an access token from Keycloak (e.g., via client credentials or token exchange) 
3. The token is used to call downstream services 
4. Services validate the token and extract claims for authorization.

### Azure environments

In Azure environments, Adaptive Apps leverages workload identity integration with Microsoft Entra ID. 
1. Kubernetes service accounts are federated with Entra ID identities
2. Pods receive tokens via workload identity (OIDC federation)
3. Tokens are used to access Azure services or other APIs securely
4. No secrets are stored in the application

## Additional Topics

* [Configure KeyCloak federation with local Active Directory](./keycloak-active-directory.md)
* [Sync local Active Directory credentials to an Azure Entra tenant](./microsoft-entra-connect.md)