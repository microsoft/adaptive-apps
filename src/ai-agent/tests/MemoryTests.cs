using System.Security.Claims;
using Adaptive.AdvisorMemory;
using Azure.Data.Tables;
using Npgsql;
using Xunit;

public sealed class MemoryTests
{
    public const string Tenant = "11111111-1111-1111-1111-111111111111";
    public const string Owner = "22222222-2222-2222-2222-222222222222";
    public const string Other = "33333333-3333-3333-3333-333333333333";
    public static ClaimsPrincipal User(string subject = Owner, string issuer = "aks", string tenant = Tenant) =>
        new(new ClaimsIdentity(new[] {
            new Claim("entra_tid", tenant), new Claim("entra_oid", subject), new Claim("iss", issuer)
        }, "validated-jwt"));
    public static MemoryScope Scope(string subject = Owner) =>
        MemoryScope.ForUser(User(subject), Tenant, "demo-source", "trading-advisor");

    [Fact]
    public void IdentityIsPortableButUserAgentAndSourceAreIsolated()
    {
        var scope = Scope();
        Assert.Equal(scope, MemoryScope.ForUser(User(issuer: "local"), Tenant, "demo-source", "trading-advisor"));
        Assert.NotEqual(scope.PartitionKey, Scope(Other).PartitionKey);
        Assert.NotEqual(scope.PartitionKey, MemoryScope.ForUser(User(), Tenant, "other-source", "trading-advisor").PartitionKey);
        Assert.NotEqual(scope.PartitionKey, MemoryScope.ForUser(User(), Tenant, "demo-source", "other-agent").PartitionKey);
        Assert.Throws<UnauthorizedAccessException>(() =>
            MemoryScope.ForUser(User(tenant: Other), Tenant, "demo-source", "trading-advisor"));
        Assert.Throws<UnauthorizedAccessException>(() =>
            MemoryScope.ForUser(new ClaimsPrincipal(new ClaimsIdentity()), Tenant, "demo-source", "trading-advisor"));
        var duplicate = User();
        ((ClaimsIdentity)duplicate.Identity!).AddClaim(new("entra_oid", Other));
        Assert.Throws<UnauthorizedAccessException>(() =>
            MemoryScope.ForUser(duplicate, Tenant, "demo-source", "trading-advisor"));
    }

    [Fact]
    public async Task AdvisorAcknowledgesOnlyCommittedFactsAndLocalWritesAreExplicitlyRejected()
    {
        var store = new TestMemoryStore();
        var service = new AdvisorMemoryService(store, Tenant, "demo-source", "trading-advisor");
        var saved = await service.PrepareAsync(User(), "Remember: The king has donkey ears!", default);
        Assert.Equal("Saved to your advisor memory.", saved.Answer);
        Assert.Single(store.Facts);
        var recalled = await service.PrepareAsync(User(issuer: "local"), "What did I ask you to remember?", default);
        Assert.Contains("The king has donkey ears!", recalled.Answer);
        var other = await service.PrepareAsync(User(Other), "Recall memories", default);
        Assert.DoesNotContain("donkey", other.Answer);
        store.ReadOnly = true;
        await Assert.ThrowsAsync<MemoryReadOnlyException>(() =>
            service.PrepareAsync(User(), "Remember: another fact", default));
        Assert.Single(store.Facts);
        store.ReadOnly = false;
        store.FailWrites = true;
        await Assert.ThrowsAsync<IOException>(() =>
            service.PrepareAsync(User(), "Remember: not saved", default));
        Assert.Single(store.Facts);
        await Assert.ThrowsAsync<ArgumentException>(() => service.PrepareAsync(User(), "Remember: ", default));
    }

    [StorageFact]
    public async Task SourceToReplicaIntegration()
    {
        // The integration job supplies isolated Azurite and PostgreSQL instances.
        var tableConnection = Environment.GetEnvironmentVariable("TEST_MEMORY_TABLE_CONNECTION");
        var postgres = Environment.GetEnvironmentVariable("TEST_MEMORY_POSTGRES");
        Assert.False(string.IsNullOrWhiteSpace(tableConnection));
        Assert.False(string.IsNullOrWhiteSpace(postgres));
        var table = new TableClient(tableConnection, "Memory" + Guid.NewGuid().ToString("N"));
        await table.CreateAsync();
        var sourceDeleted = false;
        await using var dataSource = NpgsqlDataSource.Create(postgres!);
        try
        {
            var scope = MemoryScope.ForUser(User(), Tenant, "table-test-" + Guid.NewGuid().ToString("N"), "trading-advisor");
            var source = new TableMemoryStore(table, scope.SourceId, scope.AgentId);
            var replica = new MemoryReplicaWriter(dataSource, scope.SourceId, scope.AgentId);
            var local = new PostgresMemoryStore(dataSource, scope.SourceId, scope.AgentId);
            var fact = new MemoryFact(scope.SourceId, scope.AgentId, scope.PartitionKey,
                Guid.NewGuid().ToString("N"), "The king has donkey ears!", DateTimeOffset.UtcNow);
            await source.RememberAsync(fact, default);
            var otherAgent = new TableMemoryStore(table, scope.SourceId, "another-advisor");
            await otherAgent.RememberAsync(fact with { AgentId = "another-advisor", Id = Guid.NewGuid().ToString("N") }, default);
            Assert.Contains(await source.RecallAsync(scope, default), item => item.Id == fact.Id);
            Assert.DoesNotContain(await local.RecallAsync(scope, default), item => item.Id == fact.Id);
            for (var pass = 0; pass < 2; pass++)
            {
                Assert.Equal(1, await new MemorySynchronizer(source, replica).RunAsync(default));
            }
            Assert.Single(await local.RecallAsync(scope, default), item => item.Id == fact.Id);
            Assert.Empty(await local.RecallAsync(
                MemoryScope.ForUser(User(Other), Tenant, scope.SourceId, scope.AgentId), default));
            await Assert.ThrowsAsync<InvalidOperationException>(() => local.RememberAsync(fact, default));
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                replica.ImportAsync(fact with { Text = "unexpected overwrite" }, default));
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                replica.ImportAsync(fact with { SchemaVersion = 2 }, default));
            Assert.Equal("The king has donkey ears!",
                (await local.RecallAsync(scope, default)).Single(item => item.Id == fact.Id).Text);
            await using var completed = dataSource.CreateCommand(
                "SELECT completed_at FROM advisor_memory.sync_status WHERE source_id = $1 AND agent_id = $2");
            completed.Parameters.AddWithValue(scope.SourceId);
            completed.Parameters.AddWithValue(scope.AgentId);
            var completionBeforeFailure = await completed.ExecuteScalarAsync();
            var invalid = new TableEntity(scope.PartitionKey, Guid.NewGuid().ToString("N"))
            {
                ["SourceId"] = scope.SourceId, ["AgentId"] = scope.AgentId,
                ["Text"] = "Unsupported schema", ["CreatedAt"] = DateTimeOffset.UtcNow, ["SchemaVersion"] = 2
            };
            await table.AddEntityAsync(invalid);
            await Assert.ThrowsAsync<InvalidDataException>(() => new MemorySynchronizer(source, replica).RunAsync(default));
            Assert.Equal(completionBeforeFailure, await completed.ExecuteScalarAsync());
            invalid["SchemaVersion"] = 1;
            invalid["Text"] = 42;
            await table.UpdateEntityAsync(invalid, Azure.ETag.All);
            await Assert.ThrowsAsync<InvalidDataException>(() => new MemorySynchronizer(source, replica).RunAsync(default));
            Assert.Equal(completionBeforeFailure, await completed.ExecuteScalarAsync());
            await table.DeleteEntityAsync(invalid.PartitionKey, invalid.RowKey);
            await table.DeleteEntityAsync(scope.PartitionKey, fact.Id);
            Assert.Equal(0, await new MemorySynchronizer(source, replica).RunAsync(default));
            Assert.Contains(await local.RecallAsync(scope, default), item => item.Id == fact.Id);
            // Once replicated, recall does not need the Azure source.
            await table.DeleteAsync();
            sourceDeleted = true;
            Assert.Contains(await local.RecallAsync(scope, default), item => item.Id == fact.Id);
        }
        finally
        {
            if (!sourceDeleted) await table.DeleteAsync();
        }
    }
}

public sealed class StorageFactAttribute : FactAttribute
{
    public StorageFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("TEST_MEMORY_TABLE_CONNECTION") is null ||
            Environment.GetEnvironmentVariable("TEST_MEMORY_POSTGRES") is null)
            Skip = "Set TEST_MEMORY_TABLE_CONNECTION and TEST_MEMORY_POSTGRES to run storage integration.";
    }
}

public sealed class TestMemoryStore : IMemoryStore
{
    public bool ReadOnly { get; set; }
    public bool FailWrites { get; set; }
    public List<MemoryFact> Facts { get; } = new();
    public Task RememberAsync(MemoryFact fact, CancellationToken cancellationToken)
    {
        if (FailWrites) throw new IOException("test write failure");
        Facts.Add(fact);
        return Task.CompletedTask;
    }
    public Task<IReadOnlyList<MemoryFact>> RecallAsync(MemoryScope scope, CancellationToken cancellationToken) =>
        Task.FromResult<IReadOnlyList<MemoryFact>>(Facts.Where(fact => fact.PartitionKey == scope.PartitionKey).ToArray());
}
