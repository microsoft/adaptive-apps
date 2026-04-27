# KeyCloak federation with Active Directory

## Deploy KeyCloak

You can deploy KeyCloak by installing one of the Adaptive Apps capability portfolios. Once an Adaptive App capability profile is deployed, you can acess KeyCloak portal locally by:

1. Port forward to KeyCloak portal

    ```bash
    kubectl port-forward -n min svc/min-keycloak 8080:8080
    ```

2. Open `http://localhost:8080` and sign in with the chart values:

    - username: `admin`
    - password: `admin`

    Then, you can use KeyCloak portal to manage users and credentials.

## Setting up Active Directory Domain Services (ADDS) on a VM

See instructions [here](./adds-vm.md).

## Configuring user federation

1. Login to KeyCloak portal
2. Select **User fedreation**, then **Add Ldap providers**
3. Enter these values:

   * **Connection URL**: ldaps://`<domain controller host name>`:636

        > **NOTE**: The host name needs to be consistent with certificate subject name

   * **Enable StartTLS** : Off
   * **Use Truststore SPI**: Always
   * **Connection pooling**: On

4. Use "Test Connection" button to verify connection.
5. Enter these bind values:

   * **Bind type**: simple
   * **Bind DN**: CN=Administrator,CN=Users,DC=corp,DC=local
   * **Bind credentials**: `<Administrator password>`
6. In "LDAP searching and updating" section:

    * **Edit mode**: READ_ONLY
    * **Users DN**: CN=Users,DC=corp,DC=local
    * **Username LDAP attribute**: sAMAccountName
    * **RDN LDAP attribute**: cn
    * **UUID LDAP attribute**: objectGUID
    * **User object classes**: person, organizationalPerson, user
    * **Search scope**: One Level
7. Click "Save"
8. At the top of the screen, select "Action"->"Sync all users" to test. You should see users added/updated message.
9. In KeyCloak portal, click on "Clients" in the left panel.
10. Click on "Create client" button.
11. Enter a client id and click "Next".
12. Set "Client authentication" to On.
13. Set "Valid redriect URIs" to "http://localhost:3000/*" (this is the app frontend URL).
14. Click "Save".
15. Go to "Credentails", copy "Client Secret". You'll need to supply this to your app Radius deployment.