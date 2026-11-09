using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Azure.Core;
using Azure.Identity;

namespace Adaptive.AdvisorMemory;

public sealed record MemoryScope(string SourceId, string AgentId, string PartitionKey)
{
    public static MemoryScope ForUser(ClaimsPrincipal user, string allowedTenant, string sourceId, string agentId)
    {
        var tenant = SingleGuidClaim(user, "entra_tid");
        var subject = SingleGuidClaim(user, "entra_oid");
        if (user.Identity?.IsAuthenticated != true ||
            !Guid.TryParse(allowedTenant, out var expectedTenant) || tenant != expectedTenant)
            throw new UnauthorizedAccessException("Verified Entra tenant and object-ID claims are required.");

        var key = SHA256.HashData(Encoding.UTF8.GetBytes(
            JsonSerializer.Serialize(new[] { sourceId, agentId, tenant.ToString("D"), subject.ToString("D") })));
        return new(sourceId, agentId, Convert.ToHexString(key).ToLowerInvariant());
    }

    public static MemoryScope ForKeycloakDemo(
        ClaimsPrincipal user, string allowedSubject, string demoUserId, string sourceId, string agentId)
    {
        var subject = SingleGuidClaim(user, "sub");
        if (user.Identity?.IsAuthenticated != true ||
            !Guid.TryParse(allowedSubject, out var expectedSubject) || expectedSubject == Guid.Empty ||
            subject != expectedSubject ||
            !Guid.TryParse(demoUserId, out var stableId) || stableId == Guid.Empty)
            throw new UnauthorizedAccessException("This Keycloak user is not mapped to the configured demo identity.");
        var key = SHA256.HashData(Encoding.UTF8.GetBytes(
            JsonSerializer.Serialize(new[] { sourceId, agentId, "keycloak-demo", stableId.ToString("D") })));
        return new(sourceId, agentId, Convert.ToHexString(key).ToLowerInvariant());
    }

    private static Guid SingleGuidClaim(ClaimsPrincipal user, string name)
    {
        var values = user.FindAll(name).ToArray();
        if (values.Length != 1 || !Guid.TryParse(values[0].Value, out var value) || value == Guid.Empty)
            throw new UnauthorizedAccessException($"A single verified {name} claim is required.");
        return value;
    }
}

public sealed record MemoryFact(
    string SourceId, string AgentId, string PartitionKey, string Id,
    string Text, DateTimeOffset CreatedAt, int SchemaVersion = 1)
{
    public void Validate()
    {
        if (string.IsNullOrWhiteSpace(SourceId) || string.IsNullOrWhiteSpace(AgentId) ||
            PartitionKey.Length != 64 || !PartitionKey.All(Uri.IsHexDigit) ||
            !Guid.TryParseExact(Id, "N", out _) ||
            string.IsNullOrWhiteSpace(Text) || Text.Length > 4000 ||
            CreatedAt == default || SchemaVersion != 1)
            throw new InvalidDataException("Invalid or unsupported advisor memory record.");
    }
}

public interface IMemoryStore
{
    bool ReadOnly { get; }
    Task RememberAsync(MemoryFact fact, CancellationToken cancellationToken);
    Task<IReadOnlyList<MemoryFact>> RecallAsync(MemoryScope scope, CancellationToken cancellationToken);
}

public interface IMemorySource
{
    IAsyncEnumerable<MemoryFact> ExportAsync(CancellationToken cancellationToken);
}

public static class MemoryCredentials
{
    public static TokenCredential Create(string mode, string? clientId) => mode switch
    {
        "workload" => new WorkloadIdentityCredential(new WorkloadIdentityCredentialOptions
        {
            ClientId = Required(clientId, "Azure workload identity client ID")
        }),
        "managed" => new ManagedIdentityCredential(
            ManagedIdentityId.FromUserAssignedClientId(Required(clientId, "Azure managed identity client ID"))),
        "developer" => new AzureCliCredential(),
        _ => throw new InvalidOperationException("Memory credential must be workload, managed, or developer.")
    };

    public static string Required(string? value, string name) =>
        !string.IsNullOrWhiteSpace(value) ? value : throw new InvalidOperationException($"{name} is required.");
}
