# Put KeyCloak Instance Behind an Ingress (k3s)

Keycloak can be exposed through the existing ingress controller in the k3s cluster. k3s commonly includes Traefik as the default ingress controller, and that is sufficient for exposing Keycloak behind HTTPS. 

1. Confirm Traefik is the ingress controller

    ```bash
    kubectl get ingressclass
    kubectl get pods -A | grep traefik
    ```

    You should see an ingress class such as `tratraefik.io/ingress-controller`.

2. Configure KeyCloak for reverse proxy

    ```bash
    # Assume Keycloak is deployed to the min namespace with Min Portfolio
    kubectl -n min set env deployment/min-keycloak KC_HOSTNAME=https://keycloak.example.com:8443 KC_PROXY_HEADERS=xforwarded KC_HTTP_ENABLED=true
    ```

    This is needed when TLS terminates at the ingress and Traefik forwards HTTP to KeyCloak. KeyCloak specifically requires proxy headers for reverse-proxy deployments, and `KC_HTTP_ENABLED=true` is required for edge TLS termination.

3. Restart:

    ```bash
    kubectl -n min rollout restart deployment/min-keycloak
    kubectl -n min rollout status deployment/min-keycloak
    ````

4. Create a self-signed TLS certificate (testing only)

    Create a config file `openssl-san.cnf`:
    ```bash
    [req]
    default_bits       = 2048
    distinguished_name = req_distinguished_name
    req_extensions     = req_ext
    x509_extensions    = req_ext
    prompt             = no

    [req_distinguished_name]
    CN = keycloak.example.com

    [req_ext]
    subjectAltName = @alt_names

    [alt_names]
    DNS.1 = keycloak.example.com
    ```
    Generate the cert:
    ```bash
    openssl req -x509 -nodes -days 365 \
    -newkey rsa:2048 \
    -keyout tls.key \
    -out tls.crt \
    -config openssl-san.cnf
    ```

    >**NOTE**: There's a sample `openssl-san.cnf` under the `/docs/authentication/artifacts` folder

5. Create Kubernetes TLS secret

    ```bash
    kubectl -n min create secret tls keycloak-tls \
    --cert=tls.crt \
    --key=tls.key
    ```

6. (Optional) Trust the cert locally (so browser warning goes away)

    1. Double-click `tls.crt`
    2. Click **Install Certificate**
    3. Choose **Local Machine**
    4. Place in **Trusted Root Certification Authorities**

7. Create the Traefik Ingress definition:
    ```yaml
    apiVersion: networking.k8s.io/v1
    kind: Ingress
    metadata:
    name: keycloak
    namespace: min
    spec:
    ingressClassName: traefik
    tls:
        - hosts:
            - keycloak.example.com
        secretName: keycloak-tls
    rules:
        - host: keycloak.example.com
        http:
            paths:
            - path: /
                pathType: Prefix
                backend:
                service:
                    name: min-keycloak
                    port:
                    number: 8080
    ```
    >**NOTE**: There's a sample `traefik-ingress.yml` under the `/docs/authentication/artifacts` folder
8. Apply the ingress definition:
    ```bash
    kubectl apply -f traefik-ingress.yml
    ```
9. Check Traefik service:
    ```bash
    kubectl -n kube-system get svc traefik
    ```
    For local testing, add the Traefik external IP to hosts file (`c:\Windows\System32\drivers\etc\hosts` for Windows):
    ```bash
    <traefik-external-ip> keycloak.example.com
    ```
10. If your K3s runs on WSL and your browser is on Windows, you probably need to port-forward traefik service:
    ```
    kubectl -n kube-system port-forward svc/traefik 8443:443 8080:80
    ```
11. Open a browser and access the KeyCloak portal via: https://keycloak.example.com:8443/
