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
        var tenant = Required("ADVISOR_MEMORY_ENTRA_TENANT_ID");
        if (!Guid.TryParse(tenant, out var tenantId) || tenantId == Guid.Empty)
            throw new InvalidOperationException("ADVISOR_MEMORY_ENTRA_TENANT_ID must be a nonempty tenant GUID.");
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
            services.GetRequiredService<IMemoryStore>(), tenant, sourceId, agentId));
        return true;
    }
}
