using System.Buffers;
using System.Diagnostics;
using System.Diagnostics.Metrics;
using System.Text;
using System.Text.Json;
using Azure.Core;
using Azure.Identity;
using MQTTnet;
using MQTTnet.Formatter;
using MQTTnet.Protocol;
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

// The MQTT listener retries on its own. This is a second line of defence: a
// broker outage must never take down the REST API that serves accounts,
// orders and trades.
builder.Services.Configure<HostOptions>(options =>
    options.BackgroundServiceExceptionBehavior = BackgroundServiceExceptionBehavior.Ignore);

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

class MqttOrderListener : BackgroundService, IMqttEnhancedAuthenticationHandler
{
    // Azure Event Grid accepts Microsoft Entra tokens exclusively through MQTT v5
    // enhanced authentication (CONNECT properties "Authentication Method" and
    // "Authentication Data", plus AUTH packets for re-authentication). It never
    // inspects the CONNECT password field.
    // https://learn.microsoft.com/azure/event-grid/mqtt-client-microsoft-entra-token-and-rbac
    private const string EntraAuthenticationMethod = "OAUTH2-JWT";

    // Renew well before expiry so a slow token request cannot race the broker.
    private static readonly TimeSpan TokenRenewalMargin = TimeSpan.FromMinutes(5);
    private static readonly TimeSpan MinimumRenewalDelay = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan MaximumReconnectDelay = TimeSpan.FromMinutes(1);

    private readonly IConfiguration _configuration;
    private readonly ILogger<MqttOrderListener> _logger;
    private readonly OrderProcessor _processor;
    private readonly MqttClientFactory _clientFactory = new();
    private IMqttClient? _client;
    private byte[]? _currentToken;
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
        var settings = ReadSettings();

        var client = _clientFactory.CreateMqttClient();
        _client = client;

        client.ApplicationMessageReceivedAsync += e => HandleApplicationMessageAsync(e, stoppingToken);

        var consecutiveFailures = 0;
        while (!stoppingToken.IsCancellationRequested)
        {
            var sessionStarted = Stopwatch.GetTimestamp();
            try
            {
                await RunSessionAsync(client, settings, stoppingToken);

                // A session that ended almost immediately is a failure in
                // disguise — a token refused after CONNACK, or another pod
                // taking over the same client identifier during a rolling
                // restart. Keep backing off instead of flapping once a second.
                consecutiveFailures = Stopwatch.GetElapsedTime(sessionStarted) >= MaximumReconnectDelay
                    ? 0
                    : consecutiveFailures + 1;
            }
            catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested)
            {
                break;
            }
            catch (Exception ex)
            {
                consecutiveFailures++;
                // Never rethrow: the HTTP API must keep serving even when the
                // broker is unreachable or refuses the connection.
                _logger.LogError(ex,
                    "MQTT session failed (consecutive failures: {Failures}). Order streaming is degraded; the HTTP API remains available.",
                    consecutiveFailures);
            }

            if (stoppingToken.IsCancellationRequested)
            {
                break;
            }

            var delay = ReconnectDelay(consecutiveFailures);
            _logger.LogInformation("MQTT: reconnecting in {Delay}", delay);
            try
            {
                await Task.Delay(delay, stoppingToken);
            }
            catch (OperationCanceledException)
            {
                break;
            }
        }
    }

    private async Task RunSessionAsync(IMqttClient client, MqttSettings settings, CancellationToken cancellationToken)
    {
        var sessionEnded = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);

        // Scoped to this session: a late event from a previous connection
        // attempt must not be mistaken for the end of the current one.
        Func<MqttClientDisconnectedEventArgs, Task> onDisconnected = e =>
        {
            _logger.LogWarning(e.Exception,
                "MQTT disconnected (reason: {Reason}{ReasonString})",
                e.Reason,
                string.IsNullOrWhiteSpace(e.ReasonString) ? string.Empty : $" - {e.ReasonString}");
            sessionEnded.TrySetResult();
            return Task.CompletedTask;
        };

        client.DisconnectedAsync += onDisconnected;
        try
        {
            AccessToken? token = settings.UsesEntraAuthentication
                ? await AcquireTokenAsync(settings, cancellationToken)
                : null;

            var options = BuildOptions(settings, token);
            var connectResult = await client.ConnectAsync(options, cancellationToken);

            // A refused CONNECT does not throw: MQTTnet returns the CONNACK and
            // closes the socket. Event Grid reports the real cause here (missing
            // TopicSpaces role, bad audience, wrong client identifier), so fail
            // loudly instead of tripping over MqttClientNotConnectedException on
            // the next line.
            if (connectResult.ResultCode != MqttClientConnectResultCode.Success)
            {
                throw new InvalidOperationException(
                    $"MQTT CONNECT refused by the broker: {connectResult.ResultCode}" +
                    (string.IsNullOrWhiteSpace(connectResult.ReasonString) ? string.Empty : $" - {connectResult.ReasonString}"));
            }

            _logger.LogInformation(
                "MQTT connected to {Host}:{Port} as {ClientId} (result: {ResultCode}, auth: {AuthMethod})",
                settings.Host,
                settings.Port,
                options.ClientId,
                connectResult.ResultCode,
                settings.UsesEntraAuthentication ? EntraAuthenticationMethod : "none");

            var subscribeResult = await client.SubscribeAsync(settings.Topic, MqttQualityOfServiceLevel.AtLeastOnce, cancellationToken);

            // A denied SUBACK does not throw either, and the connection stays up.
            // Without this check the backend would sit connected and silent —
            // exactly what happens when the identity holds the Event Grid
            // TopicSpaces Publisher role but not Subscriber.
            var denied = subscribeResult.Items
                .Where(item => item.ResultCode is not (MqttClientSubscribeResultCode.GrantedQoS0
                    or MqttClientSubscribeResultCode.GrantedQoS1
                    or MqttClientSubscribeResultCode.GrantedQoS2))
                .ToList();
            if (denied.Count > 0)
            {
                throw new InvalidOperationException(
                    "MQTT SUBSCRIBE denied by the broker: " +
                    string.Join(", ", denied.Select(item => $"{item.TopicFilter.Topic} => {item.ResultCode}")));
            }

            _logger.LogInformation("Listening for orders on MQTT topic {Topic}", settings.Topic);

            while (!cancellationToken.IsCancellationRequested)
            {
                var renewalDelay = token.HasValue
                    ? RenewalDelay(token.Value.ExpiresOn)
                    : Timeout.InfiniteTimeSpan;

                using var renewalCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                var renewalDue = Task.Delay(renewalDelay, renewalCts.Token);
                var finished = await Task.WhenAny(sessionEnded.Task, renewalDue);
                renewalCts.Cancel();

                if (finished == sessionEnded.Task)
                {
                    // The broker closed the session; the caller reconnects.
                    return;
                }

                cancellationToken.ThrowIfCancellationRequested();

                // Event Grid drops a client once its token expires unless the client
                // re-authenticates with an AUTH packet carrying reason code 25.
                token = await AcquireTokenAsync(settings, cancellationToken);
                var refreshedToken = Encoding.UTF8.GetBytes(token.Value.Token);
                Volatile.Write(ref _currentToken, refreshedToken);

                await client.SendEnhancedAuthenticationExchangeDataAsync(
                    new MqttEnhancedAuthenticationExchangeData
                    {
                        ReasonCode = MqttAuthenticateReasonCode.ReAuthenticate,
                        AuthenticationData = refreshedToken
                    },
                    cancellationToken);

                _logger.LogInformation(
                    "MQTT: re-authenticated with a refreshed Entra token (expires {ExpiresOn:u})",
                    token.Value.ExpiresOn);
            }
        }
        finally
        {
            client.DisconnectedAsync -= onDisconnected;

            // MQTTnet refuses ConnectAsync on a connected client, so a session
            // that failed after CONNECT (a rejected SUBSCRIBE, for example) must
            // not leave the connection open or every retry would fail.
            if (client.IsConnected)
            {
                try
                {
                    await client.DisconnectAsync(cancellationToken: CancellationToken.None);
                }
                catch (Exception ex)
                {
                    _logger.LogDebug(ex, "MQTT: cleanup disconnect failed");
                }
            }
        }
    }

    private MqttSettings ReadSettings()
    {
        // Radius injects CONNECTION_MQTT_* from the mqttBrokers connection.
        var host = _configuration["CONNECTION_MQTT_HOST"] ?? "localhost";
        var port = int.TryParse(_configuration["CONNECTION_MQTT_PORT"], out var parsed) ? parsed : 1883;
        var topic = _configuration["MQTT_TOPIC"] ?? "orders/new";

        var authMethod = _configuration["MQTT_AUTH_METHOD"] ?? "none";
        var tokenAudience = _configuration["MQTT_TOKEN_AUDIENCE"] ?? "https://eventgrid.azure.net/";
        if (!tokenAudience.EndsWith("/", StringComparison.Ordinal))
        {
            tokenAudience += "/";
        }

        var clientIdOverride = _configuration["MQTT_CLIENT_ID"];

        return new MqttSettings(
            host,
            port,
            topic,
            string.Equals(authMethod, EntraAuthenticationMethod, StringComparison.OrdinalIgnoreCase),
            tokenAudience,
            string.IsNullOrWhiteSpace(clientIdOverride) ? null : clientIdOverride,
            _configuration["AZURE_CLIENT_ID"] ?? string.Empty,
            _configuration["AZURE_TENANT_ID"] ?? string.Empty,
            _configuration["AZURE_FEDERATED_TOKEN_FILE"] ?? string.Empty);
    }

    private MqttClientOptions BuildOptions(MqttSettings settings, AccessToken? token)
    {
        var builder = new MqttClientOptionsBuilder()
            .WithTcpServer(settings.Host, settings.Port);

        if (token is null)
        {
            Volatile.Write(ref _currentToken, null);
            return builder
                .WithClientId(settings.ClientIdOverride ?? $"backend-{Guid.NewGuid():N}")
                .Build();
        }

        var tokenBytes = Encoding.UTF8.GetBytes(token.Value.Token);
        Volatile.Write(ref _currentToken, tokenBytes);

        // Event Grid matches the CONNECT client identifier against the object ID
        // of the authenticating Entra principal, which the token carries as `oid`.
        var objectId = settings.ClientIdOverride ?? TryGetObjectId(token.Value.Token);
        if (objectId is null)
        {
            _logger.LogWarning(
                "MQTT: the access token has no 'oid' claim and MQTT_CLIENT_ID is unset. Azure Event Grid rejects clients whose CONNECT client identifier is not the Entra object ID.");
        }

        return builder
            .WithClientId(objectId ?? $"backend-{Guid.NewGuid():N}")
            // Enhanced authentication requires MQTT v5.
            .WithProtocolVersion(MqttProtocolVersion.V500)
            .WithTlsOptions(tls => tls.UseTls())
            .WithEnhancedAuthentication(EntraAuthenticationMethod, tokenBytes)
            // Without a handler MQTTnet drops the connection when the broker
            // answers a re-authentication with its own AUTH packet.
            .WithEnhancedAuthenticationHandler(this)
            .Build();
    }

    public Task HandleEnhancedAuthenticationAsync(MqttEnhancedAuthenticationEventArgs eventArgs)
    {
        if (eventArgs.ReasonCode == MqttAuthenticateReasonCode.ContinueAuthentication)
        {
            _logger.LogInformation("MQTT: broker requested continued authentication; resending the current token");
            return eventArgs.SendAsync(
                new SendMqttEnhancedAuthenticationDataOptions { Data = Volatile.Read(ref _currentToken) },
                eventArgs.CancellationToken);
        }

        _logger.LogInformation(
            "MQTT: enhanced authentication acknowledged by the broker (method: {Method}, reason: {ReasonCode})",
            eventArgs.AuthenticationMethod,
            eventArgs.ReasonCode);
        return Task.CompletedTask;
    }

    private async Task<AccessToken> AcquireTokenAsync(MqttSettings settings, CancellationToken cancellationToken)
    {
        var credential = CreateCredential(settings);
        try
        {
            var tokenRequest = new TokenRequestContext(new[] { settings.TokenAudience + ".default" });
            return await credential.GetTokenAsync(tokenRequest, cancellationToken);
        }
        catch (Exception ex) when (ex is CredentialUnavailableException or AuthenticationFailedException)
        {
            _logger.LogError(ex,
                "MQTT token acquisition failed. audience={Audience}, clientIdSet={ClientIdSet}, tenantIdSet={TenantIdSet}, federatedTokenFileSet={TokenFileSet}",
                settings.TokenAudience,
                !string.IsNullOrWhiteSpace(settings.AzureClientId),
                !string.IsNullOrWhiteSpace(settings.AzureTenantId),
                !string.IsNullOrWhiteSpace(settings.FederatedTokenFile));
            throw;
        }
    }

    private static TokenCredential CreateCredential(MqttSettings settings)
    {
        // A fresh credential per request on purpose: Azure.Identity caches tokens
        // per instance and would hand back the near-expiry token we are trying to
        // replace. Renewals are hourly, so the cost is irrelevant.
        if (!string.IsNullOrWhiteSpace(settings.AzureClientId) &&
            !string.IsNullOrWhiteSpace(settings.AzureTenantId) &&
            !string.IsNullOrWhiteSpace(settings.FederatedTokenFile))
        {
            return new WorkloadIdentityCredential(new WorkloadIdentityCredentialOptions
            {
                ClientId = settings.AzureClientId,
                TenantId = settings.AzureTenantId,
                TokenFilePath = settings.FederatedTokenFile
            });
        }

        if (!string.IsNullOrWhiteSpace(settings.AzureClientId))
        {
            // Explicitly target the configured user-assigned identity to avoid
            // ambiguous IMDS selection when multiple identities are present.
            return new ManagedIdentityCredential(settings.AzureClientId);
        }

        return new DefaultAzureCredential(new DefaultAzureCredentialOptions
        {
            ExcludeVisualStudioCredential = true,
            ExcludeAzureCliCredential = true,
            ExcludeAzurePowerShellCredential = true,
            ExcludeAzureDeveloperCliCredential = true
        });
    }

    private async Task HandleApplicationMessageAsync(MqttApplicationMessageReceivedEventArgs e, CancellationToken cancellationToken)
    {
        try
        {
            var payloadSeq = e.ApplicationMessage.Payload;
            var payload = payloadSeq.Length == 0
                ? string.Empty
                : Encoding.UTF8.GetString(payloadSeq.ToArray());
            var order = JsonSerializer.Deserialize<OrderMessage>(payload, JsonOptions);
            if (order is null)
            {
                _logger.LogWarning("Invalid order payload: {Payload}", payload);
                return;
            }

            await _processor.ProcessOrderAsync(order, cancellationToken);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to process order message");
        }
    }

    /// <summary>
    /// Reads the <c>oid</c> claim (the Entra object ID of the authenticating
    /// principal) from a JWT without validating it — the broker does that.
    /// </summary>
    private static string? TryGetObjectId(string accessToken)
    {
        var segments = accessToken.Split('.');
        if (segments.Length < 2)
        {
            return null;
        }

        try
        {
            using var payload = JsonDocument.Parse(Base64UrlDecode(segments[1]));
            return payload.RootElement.TryGetProperty("oid", out var oid) && oid.ValueKind == JsonValueKind.String
                ? oid.GetString()
                : null;
        }
        catch (Exception ex) when (ex is FormatException or JsonException)
        {
            return null;
        }
    }

    private static byte[] Base64UrlDecode(string value)
    {
        var normalized = value.Replace('-', '+').Replace('_', '/');
        var padding = (4 - (normalized.Length % 4)) % 4;
        return Convert.FromBase64String(normalized.PadRight(normalized.Length + padding, '='));
    }

    private static TimeSpan RenewalDelay(DateTimeOffset expiresOn)
    {
        var delay = expiresOn - DateTimeOffset.UtcNow - TokenRenewalMargin;
        return delay < MinimumRenewalDelay ? MinimumRenewalDelay : delay;
    }

    private static TimeSpan ReconnectDelay(int consecutiveFailures)
    {
        var seconds = Math.Min(Math.Pow(2, Math.Min(consecutiveFailures, 6)), MaximumReconnectDelay.TotalSeconds);
        return TimeSpan.FromSeconds(seconds);
    }

    public override async Task StopAsync(CancellationToken cancellationToken)
    {
        if (_client is not null)
        {
            try
            {
                await _client.DisconnectAsync(cancellationToken: cancellationToken);
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "MQTT disconnect on shutdown failed");
            }
        }
        await base.StopAsync(cancellationToken);
    }

    public override void Dispose()
    {
        _client?.Dispose();
        base.Dispose();
        GC.SuppressFinalize(this);
    }

    private sealed record MqttSettings(
        string Host,
        int Port,
        string Topic,
        bool UsesEntraAuthentication,
        string TokenAudience,
        string? ClientIdOverride,
        string AzureClientId,
        string AzureTenantId,
        string FederatedTokenFile);
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
