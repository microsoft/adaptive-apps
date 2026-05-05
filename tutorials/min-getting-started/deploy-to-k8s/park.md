
4. Access Keycloak locally:

    ```bash
    kubectl port-forward -n min svc/min-keycloak 8080:8080
    ```

    Open `http://localhost:8080` and sign in with the chart values:

    - username: `admin`
    - password: `admin`