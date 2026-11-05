using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using Adaptive.AdvisorMemory;
using Npgsql;
using Xunit;

public sealed class PostgresMemoryTests
{
    [PostgresStorageFact]
    public async Task PostgresSourceSyncsAllFactsToAnIsolatedReadOnlyReplica()
    {
        var sourceConnection = Environment.GetEnvironmentVariable("TEST_MEMORY_SOURCE_POSTGRES");
        var replicaConnection = Environment.GetEnvironmentVariable("TEST_MEMORY_POSTGRES");
        Assert.False(string.IsNullOrWhiteSpace(sourceConnection));
        Assert.False(string.IsNullOrWhiteSpace(replicaConnection));
        await using var sourceData = NpgsqlDataSource.Create(sourceConnection!);
        await using var replicaData = NpgsqlDataSource.Create(replicaConnection!);
        var sourceId = "postgres-test-" + Guid.NewGuid().ToString("N");
        const string agentId = "trading-advisor";
        var scope = MemoryScope.ForUser(MemoryTests.User(), MemoryTests.Tenant, sourceId, agentId);
        var otherScope = MemoryScope.ForUser(MemoryTests.User(MemoryTests.Other), MemoryTests.Tenant, sourceId, agentId);
        var cloud = new PostgresMemoryStore(sourceData, sourceId, agentId, readOnly: false);
        var export = new PostgresMemoryStore(sourceData, sourceId, agentId);
        var local = new PostgresMemoryStore(replicaData, sourceId, agentId);
        var writer = new MemoryReplicaWriter(replicaData, sourceId, agentId);
        var sync = new MemorySynchronizer(export, writer);
        var fact = new MemoryFact(sourceId, agentId, scope.PartitionKey,
            Guid.NewGuid().ToString("N"), "The king has donkey ears!", DateTimeOffset.UtcNow);
        await cloud.RememberAsync(fact, default);
        await cloud.RememberAsync(fact with
        {
            PartitionKey = otherScope.PartitionKey, Id = Guid.NewGuid().ToString("N"), Text = "Another user's fact"
        }, default);
        for (var index = 0; index < 100; index++)
            await cloud.RememberAsync(fact with
            {
                Id = Guid.NewGuid().ToString("N"), Text = $"Older fact {index}", CreatedAt = fact.CreatedAt.AddDays(-1)
            }, default);
        var foreign = new PostgresMemoryStore(sourceData, sourceId, "another-advisor", readOnly: false);
        await foreign.RememberAsync(fact with { AgentId = "another-advisor" }, default);
        Assert.Empty(await local.RecallAsync(scope, default));
        Assert.Equal(102, await sync.RunAsync(default));
        Assert.Equal(102, await sync.RunAsync(default));
        var recalled = await local.RecallAsync(scope, default);
        Assert.Equal(100, recalled.Count);
        Assert.Single(recalled, item => item.Id == fact.Id);
        Assert.DoesNotContain(recalled, item => item.Text == "Another user's fact");
        Assert.Equal("Another user's fact", Assert.Single(await local.RecallAsync(otherScope, default)).Text);
        await Assert.ThrowsAsync<InvalidOperationException>(() => local.RememberAsync(fact, default));
        await Assert.ThrowsAsync<InvalidDataException>(() => cloud.RememberAsync(fact with { AgentId = "wrong-agent" }, default));
        await Assert.ThrowsAsync<InvalidDataException>(() => local.RecallAsync(scope with { SourceId = "wrong-source" }, default));
        var duplicate = await Assert.ThrowsAsync<PostgresException>(() =>
            cloud.RememberAsync(fact with { Text = "overwritten" }, default));
        Assert.Equal(PostgresErrorCodes.UniqueViolation, duplicate.SqlState);

        await using var completed = replicaData.CreateCommand(
            "SELECT completed_at FROM advisor_memory.sync_status WHERE source_id = $1 AND agent_id = $2");
        completed.Parameters.AddWithValue(sourceId);
        completed.Parameters.AddWithValue(agentId);
        var beforeFailure = await completed.ExecuteScalarAsync();
        await using var corrupt = replicaData.CreateCommand(
            "UPDATE advisor_memory.facts SET fact_text = 'conflicting replica' WHERE source_id = $1 AND row_key = $2");
        corrupt.Parameters.AddWithValue(sourceId);
        corrupt.Parameters.AddWithValue(fact.Id);
        await corrupt.ExecuteNonQueryAsync();
        await Assert.ThrowsAsync<InvalidDataException>(() => sync.RunAsync(default));
        Assert.Equal(beforeFailure, await completed.ExecuteScalarAsync());
        await using var repair = replicaData.CreateCommand(
            "UPDATE advisor_memory.facts SET fact_text = $1 WHERE source_id = $2 AND row_key = $3");
        repair.Parameters.AddWithValue(fact.Text);
        repair.Parameters.AddWithValue(sourceId);
        repair.Parameters.AddWithValue(fact.Id);
        await repair.ExecuteNonQueryAsync();
        await sourceData.DisposeAsync();
        var service = new AdvisorMemoryService(local, MemoryTests.Tenant, sourceId, agentId);
        var offline = await service.PrepareAsync(MemoryTests.User(issuer: "local"), "Recall memories", default);
        Assert.Contains("The king has donkey ears!", offline.Answer);
    }

    [PostgresStorageFact]
    public async Task RealPostgresAdvisorApiWritesOnCloudAndRejectsLocalWrites()
    {
        var source = Environment.GetEnvironmentVariable("TEST_MEMORY_SOURCE_POSTGRES");
        var replica = Environment.GetEnvironmentVariable("TEST_MEMORY_POSTGRES");
        Assert.False(string.IsNullOrWhiteSpace(source));
        Assert.False(string.IsNullOrWhiteSpace(replica));
        await using var cloud = new AdvisorFactory(postgres: source, postgresReadOnly: "false");
        using var cloudClient = cloud.CreateClient();
        cloudClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", cloud.Token());
        var phrase = "Portable API fact " + Guid.NewGuid().ToString("N");
        var saved = await cloudClient.PostAsJsonAsync("/advice", new { question = "Remember: " + phrase });
        Assert.Equal(HttpStatusCode.OK, saved.StatusCode);
        await using var sourceData = NpgsqlDataSource.Create(source!);
        await using var replicaData = NpgsqlDataSource.Create(replica!);
        await new MemorySynchronizer(new PostgresMemoryStore(sourceData, "demo-source", "trading-advisor"),
            new MemoryReplicaWriter(replicaData, "demo-source", "trading-advisor")).RunAsync(default);
        await using var local = new AdvisorFactory(postgres: replica);
        using var localClient = local.CreateClient();
        localClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", local.Token());
        var recall = await localClient.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.Equal(HttpStatusCode.OK, recall.StatusCode);
        Assert.Contains(phrase, await recall.Content.ReadAsStringAsync());
        var rejected = await localClient.PostAsJsonAsync("/advice", new { question = "Remember: rejected" });
        Assert.Equal(HttpStatusCode.Conflict, rejected.StatusCode);
        localClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", local.Token(MemoryTests.Other));
        var other = await localClient.PostAsJsonAsync("/advice", new { question = "Recall memories" });
        Assert.DoesNotContain(phrase, await other.Content.ReadAsStringAsync());
    }
}

public sealed class PostgresStorageFactAttribute : FactAttribute
{
    public PostgresStorageFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("TEST_MEMORY_SOURCE_POSTGRES") is null ||
            Environment.GetEnvironmentVariable("TEST_MEMORY_POSTGRES") is null)
            Skip = "Set TEST_MEMORY_SOURCE_POSTGRES and TEST_MEMORY_POSTGRES to run PostgreSQL source integration.";
    }
}
