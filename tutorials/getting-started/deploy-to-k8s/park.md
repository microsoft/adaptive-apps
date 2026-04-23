
4. Access Keycloak locally:

    ```bash
    kubectl port-forward -n compute-core svc/compute-core-keycloak 8080:8080
    ```

    Open `http://localhost:8080` and sign in with the chart values:

    - username: `admin`
    - password: `admin`