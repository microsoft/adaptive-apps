using System.Runtime.CompilerServices;
using Azure.Data.Tables;

namespace Adaptive.AdvisorMemory;

public sealed class TableMemoryStore(TableClient table, string sourceId, string agentId) : IMemoryStore, IMemorySource
{
    public bool ReadOnly => false;

    public async Task RememberAsync(MemoryFact fact, CancellationToken cancellationToken)
    {
        fact.Validate();
        if (fact.SourceId != sourceId || fact.AgentId != agentId)
            throw new InvalidDataException("Advisor memory source does not match this provider.");
        var entity = new TableEntity(fact.PartitionKey, fact.Id)
        {
            ["SourceId"] = fact.SourceId,
            ["AgentId"] = fact.AgentId,
            ["Text"] = fact.Text,
            ["CreatedAt"] = fact.CreatedAt.ToUniversalTime(),
            ["SchemaVersion"] = fact.SchemaVersion
        };
        // Immutable inserts: existing memories are never silently overwritten.
        await table.AddEntityAsync(entity, cancellationToken);
    }

    public async Task<IReadOnlyList<MemoryFact>> RecallAsync(MemoryScope scope, CancellationToken cancellationToken)
    {
        if (scope.SourceId != sourceId || scope.AgentId != agentId)
            throw new InvalidDataException("Advisor memory scope does not match this provider.");
        var filter = TableClient.CreateQueryFilter(
            $"PartitionKey eq {scope.PartitionKey} and SourceId eq {sourceId} and AgentId eq {agentId}");
        var facts = new List<MemoryFact>();
        await foreach (var entity in table.QueryAsync<TableEntity>(filter, cancellationToken: cancellationToken))
            facts.Add(FromEntity(entity));
        return facts.OrderByDescending(fact => fact.CreatedAt).ThenBy(fact => fact.Id).Take(100).ToArray();
    }

    public async IAsyncEnumerable<MemoryFact> ExportAsync(
        [EnumeratorCancellation] CancellationToken cancellationToken)
    {
        var filter = TableClient.CreateQueryFilter($"SourceId eq {sourceId} and AgentId eq {agentId}");
        await foreach (var entity in table.QueryAsync<TableEntity>(filter, cancellationToken: cancellationToken))
            yield return FromEntity(entity);
    }

    private static MemoryFact FromEntity(TableEntity entity)
    {
        try
        {
            var fact = new MemoryFact(
                entity.GetString("SourceId"), entity.GetString("AgentId"), entity.PartitionKey, entity.RowKey,
                entity.GetString("Text"), entity.GetDateTimeOffset("CreatedAt") ?? default,
                entity.GetInt32("SchemaVersion") ?? 0);
            fact.Validate();
            return fact;
        }
        catch (Exception error) when (error is InvalidCastException or FormatException or InvalidOperationException)
        {
            throw new InvalidDataException("Stored advisor memory has invalid property types.", error);
        }
    }
}
