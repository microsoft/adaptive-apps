using System.IdentityModel.Tokens.Jwt;
using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Security.Cryptography;
using Adaptive.AdvisorMemory;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.IdentityModel.Protocols;
using Microsoft.IdentityModel.Protocols.OpenIdConnect;
using Microsoft.IdentityModel.Tokens;
using Xunit;

public sealed class AdviceApiTests
{
    [Fact]
    public async Task SharedDemoDeliberatelySharesFactsBetweenAuthenticatedUsersButNeverAnonymousRequests()
    {
        await using var app = new AdvisorFactory(identityMode: "shared-demo");
        using var client = app.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized,
            (await client.PostAsJsonAsync("/advice", new { question = "Recall memories" })).StatusCode);
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(includeIdentity: false));
        Assert.Equal(HttpStatusCode.OK,
            (await client.PostAsJsonAsync("/advice", new { question = "Remember: shared demo fact" })).StatusCode);
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(MemoryTests.Other, includeIdentity: false));
        var recall = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.OK, recall.StatusCode);
        Assert.Contains("shared demo fact", await recall.Content.ReadAsStringAsync());
        app.Store.ReadOnly = true;
        Assert.Equal(HttpStatusCode.Conflict,
            (await client.PostAsJsonAsync("/advice", new { question = "Remember: forbidden local write" })).StatusCode);
        foreach (var token in new[] { app.Token(issuer: "https://other.test"), app.Token(audience: "wrong"), app.Token(expired: true) })
        {
            client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);
            Assert.Equal(HttpStatusCode.Unauthorized,
                (await client.PostAsJsonAsync("/advice", new { question = "Recall memories" })).StatusCode);
        }
    }

    [Theory]
    [InlineData("")]
    [InlineData("invalid")]
    [InlineData("00000000-0000-0000-0000-000000000000")]
    public async Task InvalidSharedScopeFailsStartup(string scope)
    {
        await using var app = new AdvisorFactory(identityMode: "shared-demo", sharedScopeId: scope);
        Assert.Throws<InvalidOperationException>(() => app.CreateClient());
    }

    [Fact]
    public async Task KeycloakDemoUsesOnlyExplicitSubjectMappingAndStillValidatesJwt()
    {
        await using var app = new AdvisorFactory(identityMode: "keycloak-demo");
        using var client = app.CreateClient();
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(includeIdentity: false));
        var saved = await client.PostAsJsonAsync("/advice", new { question = "Remember: demo-only fact" });
        Assert.Equal(HttpStatusCode.OK, saved.StatusCode);
        var recall = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Contains("demo-only fact", await recall.Content.ReadAsStringAsync());
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(MemoryTests.Other));
        var unmapped = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.Forbidden, unmapped.StatusCode);
        Assert.Contains("configured identity mode", await unmapped.Content.ReadAsStringAsync());
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(issuer: "https://other.test"));
        Assert.Equal(HttpStatusCode.Unauthorized,
            (await client.PostAsJsonAsync("/advice", new { question = "Recall memories" })).StatusCode);
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(audience: "wrong"));
        Assert.Equal(HttpStatusCode.Unauthorized,
            (await client.PostAsJsonAsync("/advice", new { question = "Recall memories" })).StatusCode);
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(expired: true));
        Assert.Equal(HttpStatusCode.Unauthorized,
            (await client.PostAsJsonAsync("/advice", new { question = "Recall memories" })).StatusCode);
    }

    [Theory]
    [InlineData("unknown", MemoryTests.Owner)]
    [InlineData("keycloak-demo", "")]
    [InlineData("keycloak-demo", "not-a-guid")]
    public async Task InvalidDemoMappingFailsStartup(string mode, string subject)
    {
        await using var app = new AdvisorFactory(identityMode: mode, demoSubject: subject);
        Assert.Throws<InvalidOperationException>(() => app.CreateClient());
    }

    [Theory]
    [InlineData(null, true)]
    [InlineData("true", true)]
    [InlineData("false", false)]
    public async Task PostgresWriteAccessRequiresExplicitOptIn(string? setting, bool expectedReadOnly)
    {
        await using var app = new AdvisorFactory(postgres: "Host=localhost;Database=unused", postgresReadOnly: setting);
        Assert.Equal(expectedReadOnly, app.Services.GetRequiredService<IMemoryStore>().ReadOnly);
    }

    [Fact]
    public async Task InvalidPostgresWriteSettingFailsStartup()
    {
        await using var app = new AdvisorFactory(postgres: "Host=localhost;Database=unused", postgresReadOnly: "invalid");
        var error = Assert.Throws<InvalidOperationException>(() => app.CreateClient());
        Assert.Contains("ADVISOR_MEMORY_POSTGRES_READ_ONLY", error.Message);
    }

    [Fact]
    public async Task DisabledMemoryPreservesUnauthenticatedAdviceValidation()
    {
        await using var app = new AdvisorFactory(memoryEnabled: false);
        using var client = app.CreateClient();
        var response = await client.PostAsJsonAsync("/advice", new { question = "" });
        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        Assert.Contains("Please provide a question.", await response.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task MemoryRequiresValidatedTokensAndSeparatesUsersAcrossFreshRequests()
    {
        await using var app = new AdvisorFactory();
        using var client = app.CreateClient();
        var missing = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.Unauthorized, missing.StatusCode);

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token());
        var save = await client.PostAsJsonAsync("/advice", new { question = "Remember: The king has donkey ears!" });
        Assert.Equal(HttpStatusCode.OK, save.StatusCode);
        var recall = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Contains("The king has donkey ears!", await recall.Content.ReadAsStringAsync());

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(MemoryTests.Other));
        var other = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.DoesNotContain("donkey", await other.Content.ReadAsStringAsync());

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(includeIdentity: false));
        var noIdentity = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.Forbidden, noIdentity.StatusCode);

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(audience: "wrong-audience"));
        var wrongAudience = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.Unauthorized, wrongAudience.StatusCode);

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(issuer: "https://untrusted.test/realms/adaptive"));
        var wrongIssuer = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.Unauthorized, wrongIssuer.StatusCode);

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token(expired: true));
        var expired = await client.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.Unauthorized, expired.StatusCode);

        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", app.Token());
        app.Store.ReadOnly = true;
        var local = await client.PostAsJsonAsync("/advice", new { question = "Remember: rejected" });
        Assert.Equal(HttpStatusCode.Conflict, local.StatusCode);
        Assert.Single(app.Store.Facts);
    }

    [Fact]
    public async Task KeycloakSigningKeysUseInternalEndpointButRetainPublicIssuer()
    {
        var documents = new Dictionary<string, string>
        {
            ["http://keycloak.min-ai.svc.cluster.local:8080/realms/adaptive/.well-known/openid-configuration"] =
                """{"issuer":"https://localhost:8445/realms/adaptive","jwks_uri":"https://localhost:8445/realms/adaptive/protocol/openid-connect/certs"}""",
            ["http://keycloak.min-ai.svc.cluster.local:8080/realms/adaptive/protocol/openid-connect/certs"] =
                """{"keys":[]}"""
        };
        var retriever = new DocumentRetriever(documents);
        var config = await new KeycloakMemoryConfigurationRetriever("https://localhost:8445/realms/adaptive")
            .GetConfigurationAsync(documents.Keys.First(), retriever, default);
        Assert.Equal("https://localhost:8445/realms/adaptive", config.Issuer);
        Assert.Equal(2, retriever.Requests.Count);
        Assert.All(retriever.Requests, address => Assert.StartsWith("http://keycloak.min-ai.svc.cluster.local", address));
        await Assert.ThrowsAsync<InvalidDataException>(() =>
            new KeycloakMemoryConfigurationRetriever("https://other.test/realms/adaptive")
                .GetConfigurationAsync(documents.Keys.First(), retriever, default));
    }

    private sealed class DocumentRetriever(Dictionary<string, string> documents) : IDocumentRetriever
    {
        public List<string> Requests { get; } = new();
        public Task<string> GetDocumentAsync(string address, CancellationToken cancel)
        {
            Requests.Add(address);
            return Task.FromResult(documents[address]);
        }
    }
}

public sealed class AdvisorFactory(
    bool memoryEnabled = true, string? postgres = null, string? postgresReadOnly = null,
    string? identityMode = null, string demoSubject = MemoryTests.Owner,
    string sharedScopeId = MemoryTests.Tenant) : WebApplicationFactory<Program>
{
    private const string Issuer = "https://identity.test/realms/adaptive";
    private readonly RsaSecurityKey key = new(RSA.Create(2048));
    public TestMemoryStore Store { get; } = new();
    protected override void ConfigureWebHost(IWebHostBuilder builder)
    {
        builder.UseSetting("CONNECTION_AI_PROVIDER", "local");
        builder.UseSetting("ADVISOR_MEMORY_PROVIDER", memoryEnabled ? (postgres is null ? "table" : "postgres") : "none");
        if (postgres is not null)
            builder.UseSetting("ADVISOR_MEMORY_POSTGRES", postgres);
        if (postgresReadOnly is not null)
            builder.UseSetting("ADVISOR_MEMORY_POSTGRES_READ_ONLY", postgresReadOnly);
        builder.UseSetting("ADVISOR_MEMORY_SOURCE_ID", "demo-source");
        builder.UseSetting("ADVISOR_MEMORY_AGENT_ID", "trading-advisor");
        builder.UseSetting("ADVISOR_MEMORY_ENTRA_TENANT_ID", MemoryTests.Tenant);
        if (identityMode is not null)
        {
            builder.UseSetting("ADVISOR_MEMORY_IDENTITY_MODE", identityMode);
            builder.UseSetting("ADVISOR_MEMORY_DEMO_SUBJECT", demoSubject);
            builder.UseSetting("ADVISOR_MEMORY_DEMO_USER_ID", MemoryTests.Tenant);
            builder.UseSetting("ADVISOR_MEMORY_SHARED_SCOPE_ID", sharedScopeId);
        }
        builder.UseSetting("ADVISOR_MEMORY_OIDC_ISSUER", Issuer);
        builder.UseSetting("ADVISOR_MEMORY_OIDC_METADATA", Issuer + "/.well-known/openid-configuration");
        builder.UseSetting("ADVISOR_MEMORY_OIDC_AUDIENCE", "trading-advisor");
        builder.UseSetting("ADVISOR_MEMORY_TABLE_ENDPOINT", "https://memory.table.core.windows.net");
        builder.UseSetting("ADVISOR_MEMORY_TABLE_NAME", "AdvisorMemory");
        builder.UseSetting("ADVISOR_MEMORY_CREDENTIAL", "developer");
        builder.ConfigureServices(services =>
        {
            if (postgres is null)
                services.AddSingleton<IMemoryStore>(Store);
            services.PostConfigure<JwtBearerOptions>(JwtBearerDefaults.AuthenticationScheme, options =>
            {
                var config = new OpenIdConnectConfiguration { Issuer = Issuer };
                config.SigningKeys.Add(key);
                options.ConfigurationManager = new StaticConfigurationManager<OpenIdConnectConfiguration>(config);
            });
        });
    }

    public string Token(string subject = MemoryTests.Owner, bool includeIdentity = true,
        string audience = "trading-advisor", string issuer = Issuer, bool expired = false)
    {
        var claims = new List<Claim> { new("sub", subject) };
        if (includeIdentity)
        {
            claims.Add(new("entra_tid", MemoryTests.Tenant));
            claims.Add(new("entra_oid", subject));
        }
        return new JwtSecurityTokenHandler().WriteToken(new JwtSecurityToken(
            issuer, audience, claims, expires: DateTime.UtcNow.AddMinutes(expired ? -5 : 5),
            signingCredentials: new SigningCredentials(key, SecurityAlgorithms.RsaSha256)));
    }
}
