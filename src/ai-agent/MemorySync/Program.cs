using Adaptive.AdvisorMemory;
using Azure.Data.Tables;
using Npgsql;

string Required(string name) => MemoryCredentials.Required(Environment.GetEnvironmentVariable(name), name);
if (args.Any(argument => argument != "--once"))
    throw new ArgumentException("Usage: AdvisorMemorySync [--once]");
var once = args.Contains("--once");
var source = Required("ADVISOR_MEMORY_SOURCE_ID");
var agent = Required("ADVISOR_MEMORY_AGENT_ID");
var sourceProvider = Environment.GetEnvironmentVariable("ADVISOR_MEMORY_SYNC_SOURCE_PROVIDER") ?? "table";
if (sourceProvider is not ("table" or "postgres"))
    throw new InvalidOperationException("ADVISOR_MEMORY_SYNC_SOURCE_PROVIDER must be table or postgres.");
await using var sourceDataSource = sourceProvider == "postgres"
    ? NpgsqlDataSource.Create(Required("ADVISOR_MEMORY_SYNC_SOURCE_POSTGRES"))
    : null;
IMemorySource cloud;
if (sourceDataSource is not null)
{
    cloud = new PostgresMemoryStore(sourceDataSource, source, agent);
}
else
{
    var endpoint = new Uri(Required("ADVISOR_MEMORY_TABLE_ENDPOINT"));
    if (endpoint.Scheme != Uri.UriSchemeHttps)
        throw new InvalidOperationException("Azure Table memory endpoint must use HTTPS.");
    var table = new TableClient(endpoint, Required("ADVISOR_MEMORY_TABLE_NAME"),
        MemoryCredentials.Create(Required("ADVISOR_MEMORY_CREDENTIAL"), Environment.GetEnvironmentVariable("AZURE_CLIENT_ID")));
    cloud = new TableMemoryStore(table, source, agent);
}
await using var dataSource = NpgsqlDataSource.Create(Required("ADVISOR_MEMORY_POSTGRES"));
var replica = new MemoryReplicaWriter(dataSource, source, agent);
var synchronizer = new MemorySynchronizer(cloud, replica);
var interval = int.Parse(Environment.GetEnvironmentVariable("ADVISOR_MEMORY_SYNC_INTERVAL_SECONDS") ?? "30");
if (interval < 1)
    throw new InvalidOperationException("Synchronization interval must be positive.");
using var stop = new CancellationTokenSource();
Console.CancelKeyPress += (_, eventArgs) => { eventArgs.Cancel = true; stop.Cancel(); };

do
{
    try
    {
        var count = await synchronizer.RunAsync(stop.Token);
        Console.WriteLine($"{DateTimeOffset.UtcNow:O} Memory sync completed: {count} records scanned.");
    }
    catch (OperationCanceledException) when (stop.IsCancellationRequested)
    {
        break;
    }
    catch (Exception error) when (!once && error is (Azure.RequestFailedException or NpgsqlException { IsTransient: true }))
    {
        Console.Error.WriteLine($"{DateTimeOffset.UtcNow:O} Memory sync failed ({error.GetType().Name}); no completion recorded. {error.Message}");
    }
    if (once) break;
    try { await Task.Delay(TimeSpan.FromSeconds(interval), stop.Token); }
    catch (OperationCanceledException) when (stop.IsCancellationRequested) { break; }
} while (!stop.IsCancellationRequested);
