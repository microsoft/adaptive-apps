using Adaptive.AdvisorMemory;
using Azure.Data.Tables;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.IdentityModel.Tokens;
using Microsoft.IdentityModel.Protocols;
using Microsoft.IdentityModel.Protocols.OpenIdConnect;
using Npgsql;

public static class AdvisorMemoryConfiguration
{
    public static bool AddAdvisorMemory(this WebApplicationBuilder builder)
    {
        var mode = builder.Configuration["ADVISOR_MEMORY_PROVIDER"] ?? "none";
        if (mode == "none") return false;
        if (mode is not ("table" or "postgres"))
            throw new InvalidOperationException("ADVISOR_MEMORY_PROVIDER must be none, table, or postgres.");

        string Required(string name) => MemoryCredentials.Required(builder.Configuration[name], name);
        var sourceId = Required("ADVISOR_MEMORY_SOURCE_ID");
        var agentId = Required("ADVISOR_MEMORY_AGENT_ID");
        var identityMode = builder.Configuration["ADVISOR_MEMORY_IDENTITY_MODE"] ?? "entra";
        if (identityMode is not ("entra" or "keycloak-demo"))
            throw new InvalidOperationException("ADVISOR_MEMORY_IDENTITY_MODE must be entra or keycloak-demo.");
        var tenant = identityMode == "entra" ? Required("ADVISOR_MEMORY_ENTRA_TENANT_ID") : "";
        var demoSubject = identityMode == "keycloak-demo" ? Required("ADVISOR_MEMORY_DEMO_SUBJECT") : null;
        var demoUserId = identityMode == "keycloak-demo" ? Required("ADVISOR_MEMORY_DEMO_USER_ID") : null;
        foreach (var (name, value) in identityMode == "entra"
            ? new[] { ("ADVISOR_MEMORY_ENTRA_TENANT_ID", tenant) }
            : new[] { ("ADVISOR_MEMORY_DEMO_SUBJECT", demoSubject), ("ADVISOR_MEMORY_DEMO_USER_ID", demoUserId) })
        {
            if (!Guid.TryParse(value, out var id) || id == Guid.Empty)
                throw new InvalidOperationException($"{name} must be a nonempty GUID.");
        }
        var issuer = Required("ADVISOR_MEMORY_OIDC_ISSUER");
        var metadata = Required("ADVISOR_MEMORY_OIDC_METADATA");
        var audience = Required("ADVISOR_MEMORY_OIDC_AUDIENCE");
        if (!Uri.TryCreate(issuer, UriKind.Absolute, out var issuerUri) || issuerUri.Scheme != "https")
            throw new InvalidOperationException("Memory OIDC issuer must use HTTPS.");
        if (!Uri.TryCreate(metadata, UriKind.Absolute, out var metadataUri) ||
            (metadataUri.Scheme != "https" && !(metadataUri.Scheme == "http" &&
                (metadataUri.IsLoopback || metadataUri.Host.EndsWith(".svc.cluster.local", StringComparison.Ordinal)))))
            throw new InvalidOperationException("Memory OIDC metadata requires HTTPS or an explicit loopback/cluster-local HTTP endpoint.");

        builder.Services.AddAuthentication(JwtBearerDefaults.AuthenticationScheme).AddJwtBearer(options =>
        {
            options.MapInboundClaims = false;
            options.Authority = issuer;
            options.MetadataAddress = metadata;
            options.RequireHttpsMetadata = metadataUri.Scheme == "https";
            options.ConfigurationManager = new ConfigurationManager<OpenIdConnectConfiguration>(
                metadata, new KeycloakMemoryConfigurationRetriever(issuer),
                new HttpDocumentRetriever { RequireHttps = metadataUri.Scheme == "https" });
            options.TokenValidationParameters = new TokenValidationParameters
            {
                ValidateIssuer = true,
                ValidIssuer = issuer,
                ValidateAudience = true,
                ValidAudience = audience,
                ValidateLifetime = true,
                RequireSignedTokens = true,
                ValidateIssuerSigningKey = true,
                ClockSkew = TimeSpan.FromSeconds(30)
            };
        });
        builder.Services.AddAuthorization();
        if (mode == "table")
        {
            var endpoint = new Uri(Required("ADVISOR_MEMORY_TABLE_ENDPOINT"));
            if (endpoint.Scheme != "https")
                throw new InvalidOperationException("Memory Table endpoint must use HTTPS.");
            var table = new TableClient(endpoint, Required("ADVISOR_MEMORY_TABLE_NAME"),
                MemoryCredentials.Create(Required("ADVISOR_MEMORY_CREDENTIAL"), builder.Configuration["AZURE_CLIENT_ID"]));
            builder.Services.AddSingleton<IMemoryStore>(new TableMemoryStore(table, sourceId, agentId));
        }

        else
        {
            var readOnlySetting = builder.Configuration["ADVISOR_MEMORY_POSTGRES_READ_ONLY"] ?? "true";
            if (!bool.TryParse(readOnlySetting, out var readOnly))
                throw new InvalidOperationException("ADVISOR_MEMORY_POSTGRES_READ_ONLY must be true or false.");
            var connectionString = Required("ADVISOR_MEMORY_POSTGRES");
            builder.Services.AddSingleton(_ => NpgsqlDataSource.Create(connectionString));
            builder.Services.AddSingleton<IMemoryStore>(services => new PostgresMemoryStore(
                services.GetRequiredService<NpgsqlDataSource>(), sourceId, agentId, readOnly));
        }
        builder.Services.AddSingleton<IAdvisorMemoryService>(services => new AdvisorMemoryService(
            services.GetRequiredService<IMemoryStore>(), tenant, sourceId, agentId, identityMode, demoSubject, demoUserId));
        return true;
    }
}
