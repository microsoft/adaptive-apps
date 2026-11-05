using Microsoft.IdentityModel.Protocols;
using Microsoft.IdentityModel.Protocols.OpenIdConnect;
using Microsoft.IdentityModel.Tokens;

public sealed class KeycloakMemoryConfigurationRetriever(string issuer)
    : IConfigurationRetriever<OpenIdConnectConfiguration>
{
    public async Task<OpenIdConnectConfiguration> GetConfigurationAsync(
        string address, IDocumentRetriever retriever, CancellationToken cancellationToken)
    {
        const string discoverySuffix = "/.well-known/openid-configuration";
        const string keysSuffix = "/protocol/openid-connect/certs";
        if (!address.EndsWith(discoverySuffix, StringComparison.Ordinal))
            throw new InvalidOperationException("Memory metadata must name a realm's OIDC discovery document.");
        var metadata = OpenIdConnectConfiguration.Create(
            await retriever.GetDocumentAsync(address, cancellationToken));
        if (metadata.Issuer != issuer || metadata.JwksUri != issuer.TrimEnd('/') + keysSuffix)
            throw new InvalidDataException("Keycloak discovery issuer or signing-key endpoint does not match the reviewed issuer.");
        // Keycloak advertises its browser hostname; fetch keys over the reviewed
        // internal realm endpoint instead of trying localhost inside the advisor Pod.
        var keysAddress = address[..^discoverySuffix.Length] + keysSuffix;
        var keys = new JsonWebKeySet(await retriever.GetDocumentAsync(keysAddress, cancellationToken));
        foreach (var key in keys.GetSigningKeys()) metadata.SigningKeys.Add(key);
        return metadata;
    }
}
