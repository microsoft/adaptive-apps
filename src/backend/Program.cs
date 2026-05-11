using System.Buffers;
using System.Diagnostics;
using System.Diagnostics.Metrics;
using System.Text.Json;
using Azure.Core;
using Azure.Identity;
using MQTTnet;
using Npgsql;
using OpenTelemetry.Metrics;
using OpenTelemetry.Resources;
using OpenTelemetry.Trace;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddOpenTelemetry()
    .ConfigureResource(resource => resource
        .AddService("trading-backend", serviceVersion: "1.0.0")
        .AddAttributes(new Dictionary<string, object>
        {
            ["service.namespace"] = "portable-apps"
        }))
    .WithTracing(tracing => tracing
        .AddSource(BackendTelemetry.ActivitySourceName)
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddOtlpExporter())
    .WithMetrics(metrics => metrics
        .AddMeter(BackendTelemetry.MeterName)
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddRuntimeInstrumentation()
        .AddProcessInstrumentation()
        .AddOtlpExporter());

builder.Services.AddCors(options =>
{
    options.AddDefaultPolicy(policy =>
        policy.AllowAnyOrigin()
            .AllowAnyHeader()
            .AllowAnyMethod());
});

builder.Services.AddSingleton(sp =>
{
    // Radius injects CONNECTION_DB_* from the postgreSqlDatabases connection.
    // For local dev (docker-compose) the same env vars are set explicitly.
    var host     = builder.Configuration["CONNECTION_DB_HOST"]             ?? "localhost";
    var port     = builder.Configuration["CONNECTION_DB_PORT"]             ?? "5432";
    var database = builder.Configuration["CONNECTION_DB_DATABASE"]         ?? "trading";
    var username = builder.Configuration["CONNECTION_DB_USERNAME"]         ?? "trade";
    var password = builder.Configuration["CONNECTION_DB_SECRETS_PASSWORD"] ?? "tradepass";
    var connString = $"Host={host};Port={port};Database={database};Username={username};Password={password}";
    return NpgsqlDataSource.Create(connString);
});
builder.Services.AddSingleton<OrderProcessor>();
builder.Services.AddHostedService<MqttOrderListener>();

var app = builder.Build();
app.UseCors();

app.MapGet("/api/health", () => Results.Ok(new { status = "ok" }));

app.MapGet("/api/symbols", () =>
    Results.Ok(new[]
    {
        "AAPL", "MSFT", "GOOGL", "AMZN", "TSLA",
        "NVDA", "META", "NFLX", "AMD", "INTC"
    }));

app.MapGet("/api/orders", async (OrderProcessor processor) =>
    Results.Ok(await processor.GetOrdersAsync()));

app.MapPost("/api/orders", async (OrderMessage order, OrderProcessor processor, CancellationToken cancellationToken) =>
{
    if (string.IsNullOrWhiteSpace(order.Symbol) || order.Quantity <= 0 || order.Price <= 0)
    {
        return Results.BadRequest(new { error = "Invalid order payload" });
    }

    await processor.ProcessOrderAsync(order, cancellationToken);
    return Results.Accepted(value: new { status = "queued" });
});

app.MapGet("/api/trades", async (OrderProcessor processor) =>
    Results.Ok(await processor.GetTradesAsync()));

app.MapGet("/api/positions", async (OrderProcessor processor) =>
    Results.Ok(await processor.GetPositionsAsync()));

app.MapGet("/api/accounts", async (OrderProcessor processor) =>
    Results.Ok(await processor.GetAccountsAsync()));

app.Run();

record OrderMessage(
    string Symbol,
    string Side,
    string OrderType,
    int Quantity,
    decimal Price,
    string ClientOrderId
);

class MqttOrderListener : BackgroundService
{
    private readonly IConfiguration _configuration;
    private readonly ILogger<MqttOrderListener> _logger;
    private readonly OrderProcessor _processor;
    private IMqttClient? _client;
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    public MqttOrderListener(
        IConfiguration configuration,
        ILogger<MqttOrderListener> logger,
        OrderProcessor processor)
    {
        _configuration = configuration;
        _logger = logger;
        _processor = processor;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        // Radius injects CONNECTION_MQTT_* from the mqttBrokers connection.
        var host  = _configuration["CONNECTION_MQTT_HOST"] ?? "localhost";
        var port  = int.TryParse(_configuration["CONNECTION_MQTT_PORT"], out var parsed) ? parsed : 1883;
        var topic = _configuration["MQTT_TOPIC"] ?? "orders/new";

        var factory = new MqttClientFactory();
        _client = factory.CreateMqttClient();

        var authMethod = _configuration["MQTT_AUTH_METHOD"] ?? "none";
        var tokenAudience = _configuration["MQTT_TOKEN_AUDIENCE"] ?? "https://eventgrid.azure.net/";
        var azureClientId = _configuration["AZURE_CLIENT_ID"] ?? string.Empty;
        var azureTenantId = _configuration["AZURE_TENANT_ID"] ?? string.Empty;
        var federatedTokenFile = _configuration["AZURE_FEDERATED_TOKEN_FILE"] ?? string.Empty;

        if (!tokenAudience.EndsWith("/", StringComparison.Ordinal))
        {
            tokenAudience += "/";
        }

        var optionsBuilder = new MqttClientOptionsBuilder()
            .WithTcpServer(host, port)
            .WithClientId($"backend-{Guid.NewGuid():N}");

        if (string.Equals(authMethod, "OAUTH2-JWT", StringComparison.OrdinalIgnoreCase))
        {
            TokenCredential credential;
            if (!string.IsNullOrWhiteSpace(azureClientId) &&
                !string.IsNullOrWhiteSpace(azureTenantId) &&
                !string.IsNullOrWhiteSpace(federatedTokenFile))
            {
                credential = new WorkloadIdentityCredential(new WorkloadIdentityCredentialOptions
                {
                    ClientId = azureClientId,
                    TenantId = azureTenantId,
                    TokenFilePath = federatedTokenFile
                });
            }
            else if (!string.IsNullOrWhiteSpace(azureClientId))
            {
                // Explicitly target the configured user-assigned identity to avoid
                // ambiguous IMDS selection when multiple identities are present.
                credential = new ManagedIdentityCredential(azureClientId);
            }
            else
            {
                credential = new DefaultAzureCredential(new DefaultAzureCredentialOptions
                {
                    ExcludeVisualStudioCredential = true,
                    ExcludeAzureCliCredential = true,
                    ExcludeAzurePowerShellCredential = true,
                    ExcludeAzureDeveloperCliCredential = true
                });
            }

            AccessToken accessToken;
            try
            {
                var tokenRequest = new TokenRequestContext(new[] { tokenAudience + ".default" });
                accessToken = await credential.GetTokenAsync(tokenRequest, stoppingToken);
            }
            catch (Exception ex) when (ex is CredentialUnavailableException or AuthenticationFailedException)
            {
                _logger.LogError(ex,
                    "MQTT token acquisition failed. authMethod={AuthMethod}, clientIdSet={ClientIdSet}, tenantIdSet={TenantIdSet}, federatedTokenFileSet={TokenFileSet}",
                    authMethod,
                    !string.IsNullOrWhiteSpace(azureClientId),
                    !string.IsNullOrWhiteSpace(azureTenantId),
                    !string.IsNullOrWhiteSpace(federatedTokenFile));
                return;
            }

            optionsBuilder = optionsBuilder
                .WithTlsOptions(tls => tls.UseTls())
                // MQTTnet v5 uses username/password credentials for broker auth.
                // Event Grid validates the JWT bearer token provided as password.
                .WithCredentials(
                    string.IsNullOrWhiteSpace(azureClientId) ? "oauth2-jwt" : azureClientId,
                    accessToken.Token);

            _logger.LogInformation("MQTT: using OAUTH2-JWT authentication (audience: {Audience})", tokenAudience);
        }

        var options = optionsBuilder.Build();

        _client.ApplicationMessageReceivedAsync += async e =>
        {
            try
            {
                var payloadSeq = e.ApplicationMessage.Payload;
                var payload = payloadSeq.Length == 0
                    ? string.Empty
                    : System.Text.Encoding.UTF8.GetString(payloadSeq.ToArray());
                var order = JsonSerializer.Deserialize<OrderMessage>(payload, JsonOptions);
                if (order is null)
                {
                    _logger.LogWarning("Invalid order payload: {Payload}", payload);
                    return;
                }

                await _processor.ProcessOrderAsync(order, stoppingToken);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to process order message");
            }
        };

        await _client.ConnectAsync(options, stoppingToken);
        await _client.SubscribeAsync(topic, MQTTnet.Protocol.MqttQualityOfServiceLevel.AtLeastOnce, stoppingToken);

        _logger.LogInformation("Listening for orders on MQTT topic {Topic}", topic);
    }

    public override async Task StopAsync(CancellationToken cancellationToken)
    {
        if (_client is not null)
        {
            await _client.DisconnectAsync(cancellationToken: cancellationToken);
        }
        await base.StopAsync(cancellationToken);
    }
}

class OrderProcessor
{
    private readonly NpgsqlDataSource _dataSource;
    private readonly ILogger<OrderProcessor> _logger;

    public OrderProcessor(NpgsqlDataSource dataSource, ILogger<OrderProcessor> logger)
    {
        _dataSource = dataSource;
        _logger = logger;
    }

    public async Task ProcessOrderAsync(OrderMessage order, CancellationToken cancellationToken)
    {
        using var activity = BackendTelemetry.ActivitySource.StartActivity("process.order", ActivityKind.Internal);
        activity?.SetTag("order.symbol", order.Symbol);
        activity?.SetTag("order.side", order.Side);
        activity?.SetTag("order.type", order.OrderType);

        var started = Stopwatch.GetTimestamp();
        await Task.Delay(TimeSpan.FromMilliseconds(200), cancellationToken);

        await using var conn = await _dataSource.OpenConnectionAsync(cancellationToken);
        await using var tx = await conn.BeginTransactionAsync(cancellationToken);

        const int accountId = 1;

        // Validate sell orders: ensure sufficient position
        if (string.Equals(order.Side, "SELL", StringComparison.OrdinalIgnoreCase))
        {
            await using var checkPos = new NpgsqlCommand(@"
                SELECT COALESCE(quantity, 0) FROM positions
                WHERE account_id = @account_id AND symbol = @symbol;", conn, tx);
            checkPos.Parameters.AddWithValue("account_id", accountId);
            checkPos.Parameters.AddWithValue("symbol", order.Symbol);
            var result = await checkPos.ExecuteScalarAsync(cancellationToken);
            var held = result is int q ? q : 0;

            if (held < order.Quantity)
            {
                // Reject the order — insert with 'rejected' status, skip trade/position
                await using var rejectOrder = new NpgsqlCommand(@"
                    INSERT INTO orders (account_id, symbol, side, order_type, quantity, price, status)
                    VALUES (@account_id, @symbol, @side, @order_type, @quantity, @price, 'rejected');", conn, tx);
                rejectOrder.Parameters.AddWithValue("account_id", accountId);
                rejectOrder.Parameters.AddWithValue("symbol", order.Symbol);
                rejectOrder.Parameters.AddWithValue("side", order.Side.ToUpperInvariant());
                rejectOrder.Parameters.AddWithValue("order_type", (order.OrderType ?? "MARKET").ToUpperInvariant());
                rejectOrder.Parameters.AddWithValue("quantity", order.Quantity);
                rejectOrder.Parameters.AddWithValue("price", order.Price);
                await rejectOrder.ExecuteNonQueryAsync(cancellationToken);
                await tx.CommitAsync(cancellationToken);

                _logger.LogWarning(
                    "Rejected SELL order {ClientOrderId} for {Symbol}: requested {Requested}, held {Held}",
                    order.ClientOrderId, order.Symbol, order.Quantity, held);
                BackendTelemetry.OrderRejectedCounter.Add(1,
                    new KeyValuePair<string, object?>("symbol", order.Symbol),
                    new KeyValuePair<string, object?>("side", order.Side.ToUpperInvariant()));
                BackendTelemetry.OrderProcessDurationMs.Record(GetElapsedMilliseconds(started),
                    new KeyValuePair<string, object?>("status", "rejected"));
                return;
            }
        }

        await using var insertOrder = new NpgsqlCommand(@"
            INSERT INTO orders (account_id, symbol, side, order_type, quantity, price, status)
            VALUES (@account_id, @symbol, @side, @order_type, @quantity, @price, 'processed')
            RETURNING id;", conn, tx);
        insertOrder.Parameters.AddWithValue("account_id", accountId);
        insertOrder.Parameters.AddWithValue("symbol", order.Symbol);
        insertOrder.Parameters.AddWithValue("side", order.Side.ToUpperInvariant());
        insertOrder.Parameters.AddWithValue("order_type", (order.OrderType ?? "MARKET").ToUpperInvariant());
        insertOrder.Parameters.AddWithValue("quantity", order.Quantity);
        insertOrder.Parameters.AddWithValue("price", order.Price);

        var orderId = (int)await insertOrder.ExecuteScalarAsync(cancellationToken);

        await using var insertTrade = new NpgsqlCommand(@"
            INSERT INTO trades (order_id, account_id, symbol, side, quantity, price)
            VALUES (@order_id, @account_id, @symbol, @side, @quantity, @price);", conn, tx);
        insertTrade.Parameters.AddWithValue("order_id", orderId);
        insertTrade.Parameters.AddWithValue("account_id", accountId);
        insertTrade.Parameters.AddWithValue("symbol", order.Symbol);
        insertTrade.Parameters.AddWithValue("side", order.Side.ToUpperInvariant());
        insertTrade.Parameters.AddWithValue("quantity", order.Quantity);
        insertTrade.Parameters.AddWithValue("price", order.Price);
        await insertTrade.ExecuteNonQueryAsync(cancellationToken);

        if (string.Equals(order.Side, "BUY", StringComparison.OrdinalIgnoreCase))
        {
            await using var upsertPosition = new NpgsqlCommand(@"
                INSERT INTO positions (account_id, symbol, quantity, avg_price)
                VALUES (@account_id, @symbol, @quantity, @price)
                ON CONFLICT (account_id, symbol) DO UPDATE
                SET quantity = positions.quantity + EXCLUDED.quantity,
                    avg_price = ((positions.avg_price * positions.quantity) + (EXCLUDED.avg_price * EXCLUDED.quantity))
                        / NULLIF(positions.quantity + EXCLUDED.quantity, 0),
                    updated_at = NOW();", conn, tx);
            upsertPosition.Parameters.AddWithValue("account_id", accountId);
            upsertPosition.Parameters.AddWithValue("symbol", order.Symbol);
            upsertPosition.Parameters.AddWithValue("quantity", order.Quantity);
            upsertPosition.Parameters.AddWithValue("price", order.Price);
            await upsertPosition.ExecuteNonQueryAsync(cancellationToken);
        }
        else
        {
            await using var sellPosition = new NpgsqlCommand(@"
                INSERT INTO positions (account_id, symbol, quantity, avg_price)
                VALUES (@account_id, @symbol, -@quantity, @price)
                ON CONFLICT (account_id, symbol) DO UPDATE
                SET quantity = positions.quantity - @quantity,
                    avg_price = CASE
                        WHEN positions.quantity - @quantity = 0 THEN 0
                        ELSE positions.avg_price
                    END,
                    updated_at = NOW();", conn, tx);
            sellPosition.Parameters.AddWithValue("account_id", accountId);
            sellPosition.Parameters.AddWithValue("symbol", order.Symbol);
            sellPosition.Parameters.AddWithValue("quantity", order.Quantity);
            sellPosition.Parameters.AddWithValue("price", order.Price);
            await sellPosition.ExecuteNonQueryAsync(cancellationToken);
        }

        await tx.CommitAsync(cancellationToken);

        _logger.LogInformation("Processed order {ClientOrderId} for {Symbol}", order.ClientOrderId, order.Symbol);
        BackendTelemetry.OrderProcessedCounter.Add(1,
            new KeyValuePair<string, object?>("symbol", order.Symbol),
            new KeyValuePair<string, object?>("side", order.Side.ToUpperInvariant()));
        BackendTelemetry.OrderProcessDurationMs.Record(GetElapsedMilliseconds(started),
            new KeyValuePair<string, object?>("status", "processed"));
    }

    private static double GetElapsedMilliseconds(long startedTimestamp)
    {
        var elapsed = Stopwatch.GetElapsedTime(startedTimestamp);
        return elapsed.TotalMilliseconds;
    }

    public async Task<IEnumerable<object>> GetOrdersAsync()
    {
        await using var conn = await _dataSource.OpenConnectionAsync();
        await using var cmd = new NpgsqlCommand(@"
            SELECT id, symbol, side, order_type, quantity, price, status, created_at
            FROM orders
            ORDER BY created_at DESC
            LIMIT 100;", conn);

        var results = new List<object>();
        await using var reader = await cmd.ExecuteReaderAsync();
        while (await reader.ReadAsync())
        {
            results.Add(new
            {
                id = reader.GetInt32(0),
                symbol = reader.GetString(1),
                side = reader.GetString(2),
                orderType = reader.GetString(3),
                quantity = reader.GetInt32(4),
                price = reader.GetDecimal(5),
                status = reader.GetString(6),
                createdAt = reader.GetDateTime(7)
            });
        }

        return results;
    }

    public async Task<IEnumerable<object>> GetTradesAsync()
    {
        await using var conn = await _dataSource.OpenConnectionAsync();
        await using var cmd = new NpgsqlCommand(@"
            SELECT id, symbol, side, quantity, price, executed_at
            FROM trades
            ORDER BY executed_at DESC
            LIMIT 100;", conn);

        var results = new List<object>();
        await using var reader = await cmd.ExecuteReaderAsync();
        while (await reader.ReadAsync())
        {
            results.Add(new
            {
                id = reader.GetInt32(0),
                symbol = reader.GetString(1),
                side = reader.GetString(2),
                quantity = reader.GetInt32(3),
                price = reader.GetDecimal(4),
                executedAt = reader.GetDateTime(5)
            });
        }

        return results;
    }

    public async Task<IEnumerable<object>> GetPositionsAsync()
    {
        await using var conn = await _dataSource.OpenConnectionAsync();
        await using var cmd = new NpgsqlCommand(@"
            SELECT symbol, quantity, avg_price, updated_at
            FROM positions
            ORDER BY symbol;", conn);

        var results = new List<object>();
        await using var reader = await cmd.ExecuteReaderAsync();
        while (await reader.ReadAsync())
        {
            results.Add(new
            {
                symbol = reader.GetString(0),
                quantity = reader.GetInt32(1),
                avgPrice = reader.GetDecimal(2),
                updatedAt = reader.GetDateTime(3)
            });
        }

        return results;
    }

    public async Task<IEnumerable<object>> GetAccountsAsync()
    {
        await using var conn = await _dataSource.OpenConnectionAsync();
        await using var cmd = new NpgsqlCommand(@"
            SELECT id, name, cash_balance
            FROM accounts
            ORDER BY id;", conn);

        var results = new List<object>();
        await using var reader = await cmd.ExecuteReaderAsync();
        while (await reader.ReadAsync())
        {
            results.Add(new
            {
                id = reader.GetInt32(0),
                name = reader.GetString(1),
                cashBalance = reader.GetDecimal(2)
            });
        }

        return results;
    }
}

static class BackendTelemetry
{
    public const string ActivitySourceName = "portable-apps.backend";
    public const string MeterName = "portable-apps.backend.metrics";

    public static readonly ActivitySource ActivitySource = new(ActivitySourceName);
    private static readonly Meter Meter = new(MeterName);

    public static readonly Counter<long> OrderProcessedCounter = Meter.CreateCounter<long>(
        "backend_orders_processed_total",
        description: "Total number of successfully processed orders");

    public static readonly Counter<long> OrderRejectedCounter = Meter.CreateCounter<long>(
        "backend_orders_rejected_total",
        description: "Total number of rejected orders");

    public static readonly Histogram<double> OrderProcessDurationMs = Meter.CreateHistogram<double>(
        "backend_order_processing_duration_ms",
        unit: "ms",
        description: "Order processing duration in milliseconds");
}
