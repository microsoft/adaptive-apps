using System.Runtime.CompilerServices;
using Npgsql;

namespace Adaptive.AdvisorMemory;

public sealed class PostgresMemoryStore(
    NpgsqlDataSource dataSource, string sourceId, string agentId, bool readOnly = true) : IMemoryStore, IMemorySource
{
    public bool ReadOnly => readOnly;

    public async Task RememberAsync(MemoryFact fact, CancellationToken cancellationToken)
    {
        if (ReadOnly)
            throw new InvalidOperationException("Local advisor memory is read-only. Remember facts on the cloud advisor.");
        fact.Validate();
        ValidateScope(fact.SourceId, fact.AgentId);
        await using var command = dataSource.CreateCommand("""
            INSERT INTO advisor_memory.facts
                (source_id, agent_id, partition_key, row_key, fact_text, created_at, schema_version)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            """);
        AddFactParameters(command, fact);
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task<IReadOnlyList<MemoryFact>> RecallAsync(MemoryScope scope, CancellationToken cancellationToken)
    {
        ValidateScope(scope.SourceId, scope.AgentId);
        await using var command = dataSource.CreateCommand("""
            SELECT partition_key, row_key, fact_text, created_at, schema_version
            FROM advisor_memory.facts
            WHERE source_id = $1 AND agent_id = $2 AND partition_key = $3
            ORDER BY created_at DESC, row_key LIMIT 100
            """);
        command.Parameters.AddWithValue(scope.SourceId);
        command.Parameters.AddWithValue(scope.AgentId);
        command.Parameters.AddWithValue(scope.PartitionKey);
        var facts = new List<MemoryFact>();
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        while (await reader.ReadAsync(cancellationToken))
            facts.Add(ReadFact(reader));
        return facts;
    }

    public async IAsyncEnumerable<MemoryFact> ExportAsync(
        [EnumeratorCancellation] CancellationToken cancellationToken)
    {
        await using var command = dataSource.CreateCommand("""
            SELECT partition_key, row_key, fact_text, created_at, schema_version
            FROM advisor_memory.facts
            WHERE source_id = $1 AND agent_id = $2
            ORDER BY partition_key, row_key
            """);
        command.Parameters.AddWithValue(sourceId);
        command.Parameters.AddWithValue(agentId);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        while (await reader.ReadAsync(cancellationToken))
            yield return ReadFact(reader);
    }

    private MemoryFact ReadFact(NpgsqlDataReader reader)
    {
        var fact = new MemoryFact(sourceId, agentId, reader.GetString(0), reader.GetString(1),
            reader.GetString(2), reader.GetFieldValue<DateTimeOffset>(3), reader.GetInt32(4));
        fact.Validate();
        return fact;
    }

    private void ValidateScope(string factSourceId, string factAgentId)
    {
        if (factSourceId != sourceId || factAgentId != agentId)
            throw new InvalidDataException("Advisor memory scope does not match this provider.");
    }

    internal static void AddFactParameters(NpgsqlCommand command, MemoryFact fact)
    {
        command.Parameters.AddWithValue(fact.SourceId);
        command.Parameters.AddWithValue(fact.AgentId);
        command.Parameters.AddWithValue(fact.PartitionKey);
        command.Parameters.AddWithValue(fact.Id);
        command.Parameters.AddWithValue(fact.Text);
        command.Parameters.AddWithValue(fact.CreatedAt.ToUniversalTime());
        command.Parameters.AddWithValue(fact.SchemaVersion);
    }
}

public sealed class MemoryReplicaWriter(NpgsqlDataSource dataSource, string sourceId, string agentId)
{
    public async Task ImportAsync(MemoryFact fact, CancellationToken cancellationToken)
    {
        fact.Validate();
        if (fact.SourceId != sourceId || fact.AgentId != agentId)
            throw new InvalidDataException("Imported memory has an unexpected source or agent.");
        await using var command = dataSource.CreateCommand("""
            INSERT INTO advisor_memory.facts
                (source_id, agent_id, partition_key, row_key, fact_text, created_at, schema_version)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (source_id, agent_id, partition_key, row_key) DO UPDATE
                SET row_key = EXCLUDED.row_key
                WHERE advisor_memory.facts.fact_text = EXCLUDED.fact_text
                  AND advisor_memory.facts.created_at = EXCLUDED.created_at
                  AND advisor_memory.facts.schema_version = EXCLUDED.schema_version
            """);
        PostgresMemoryStore.AddFactParameters(command, fact);
        if (await command.ExecuteNonQueryAsync(cancellationToken) != 1)
            throw new InvalidDataException("Immutable memory changed at the source; synchronization stopped.");
    }

    public async Task CompleteAsync(long count, CancellationToken cancellationToken)
    {
        await using var command = dataSource.CreateCommand("""
            INSERT INTO advisor_memory.sync_status (source_id, agent_id, completed_at, scanned_count)
            VALUES ($1, $2, now(), $3)
            ON CONFLICT (source_id, agent_id) DO UPDATE
                SET completed_at = EXCLUDED.completed_at, scanned_count = EXCLUDED.scanned_count
            """);
        command.Parameters.AddWithValue(sourceId);
        command.Parameters.AddWithValue(agentId);
        command.Parameters.AddWithValue(count);
        await command.ExecuteNonQueryAsync(cancellationToken);
    }
}

public sealed class MemorySynchronizer(IMemorySource source, MemoryReplicaWriter replica)
{
    public async Task<long> RunAsync(CancellationToken cancellationToken)
    {
        long count = 0;
        await foreach (var fact in source.ExportAsync(cancellationToken))
        {
            await replica.ImportAsync(fact, cancellationToken);
            count++;
        }
        await replica.CompleteAsync(count, cancellationToken);
        return count;
    }
}
