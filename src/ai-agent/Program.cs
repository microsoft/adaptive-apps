using System.ComponentModel;
using System.Diagnostics;
using System.Diagnostics.Metrics;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Agents.AI;
using Microsoft.Extensions.AI;
using OpenAI;
using OpenTelemetry.Metrics;
using OpenTelemetry.Resources;
using OpenTelemetry.Trace;

// ---------------------------------------------------------------------------
// Configuration
//
//   AI_PROVIDER   = "openai" | "azure" | "azure-key" | "local"  (default: "openai")
//   OPENAI_API_KEY = your OpenAI key               (required for "openai" & "azure-key")
//   AI_ENDPOINT    = custom base URL                (required for "azure", "azure-key" & "local")
//                     Kaito example: http://workspace-trading-advisor:80/v1
//   AI_MODEL       = deployment / model name        (default: gpt-4o)
// ---------------------------------------------------------------------------

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddOpenTelemetry()
    .ConfigureResource(resource => resource
        .AddService("trading-ai-agent", serviceVersion: "1.0.0")
        .AddAttributes(new Dictionary<string, object>
        {
            ["service.namespace"] = "portable-apps"
        }))
    .WithTracing(tracing => tracing
        .AddSource(AiAgentTelemetry.ActivitySourceName)
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddOtlpExporter())
    .WithMetrics(metrics => metrics
        .AddMeter(AiAgentTelemetry.MeterName)
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddRuntimeInstrumentation()
        .AddProcessInstrumentation()
        .AddOtlpExporter());

builder.Services.AddCors(options =>
    options.AddDefaultPolicy(policy =>
        policy.AllowAnyOrigin().AllowAnyHeader().AllowAnyMethod()));

// --- Build the AI Agent as a singleton service --------------------------------

// Radius injects CONNECTION_AI_* from the aiModels connection.
// For local dev (docker-compose) the same env vars are set explicitly.
var provider = (Environment.GetEnvironmentVariable("CONNECTION_AI_PROVIDER") ?? "openai").ToLowerInvariant();
var apiKey   = Environment.GetEnvironmentVariable("CONNECTION_AI_SECRETS_APIKEY") ?? "";
var endpoint = Environment.GetEnvironmentVariable("CONNECTION_AI_ENDPOINT") ?? "";
var model    = Environment.GetEnvironmentVariable("CONNECTION_AI_MODEL") ?? "gpt-4o";

// Strip any "publisher/" prefix (e.g. "Qwen/Qwen3-0.6B" -> "Qwen3-0.6B")
var slashIdx = model.LastIndexOf('/');
if (slashIdx >= 0 && slashIdx < model.Length - 1)
{
    model = model[(slashIdx + 1)..];
}

AIAgent agent;

switch (provider)
{
    // ── Azure OpenAI (Entra / managed identity) ──────────────────────
    case "azure":
    {
        if (string.IsNullOrEmpty(endpoint))
            throw new InvalidOperationException("AI_ENDPOINT is required when AI_PROVIDER=azure");

        var azureClient = new Azure.AI.OpenAI.AzureOpenAIClient(
            new Uri(endpoint),
            new Azure.Identity.DefaultAzureCredential());

        agent = azureClient
            .GetChatClient(model)
            .AsIChatClient()
            .AsAIAgent(
                instructions: AgentInstructions.Value,
                name: "TradingAdvisor",
                tools: AgentTools.All);
        break;
    }

    // ── Azure OpenAI (API key) ───────────────────────────────────────
    case "azure-key":
    {
        if (string.IsNullOrEmpty(endpoint))
            throw new InvalidOperationException("AI_ENDPOINT is required when AI_PROVIDER=azure-key");
        if (string.IsNullOrEmpty(apiKey))
            throw new InvalidOperationException("API key is required when AI_PROVIDER=azure-key");

        var azureKeyClient = new Azure.AI.OpenAI.AzureOpenAIClient(
            new Uri(endpoint),
            new System.ClientModel.ApiKeyCredential(apiKey));

        agent = azureKeyClient
            .GetChatClient(model)
            .AsIChatClient()
            .AsAIAgent(
                instructions: AgentInstructions.Value,
                name: "TradingAdvisor",
                tools: AgentTools.All);
        break;
    }

    // ── Local / Kaito / vLLM / Ollama (OpenAI-compatible) ────────────
    case "local":
    {
        var localEndpoint = string.IsNullOrEmpty(endpoint) ? "http://localhost:80/v1" : endpoint;
        var opts = new OpenAIClientOptions { Endpoint = new Uri(localEndpoint) };

        // Local endpoints typically don't require a real API key
        var localKey = string.IsNullOrEmpty(apiKey) ? "not-needed" : apiKey;
        var localClient = new OpenAIClient(new System.ClientModel.ApiKeyCredential(localKey), opts);

        agent = localClient
            .GetChatClient(model)
            .AsIChatClient()
            .AsAIAgent(
                instructions: AgentInstructions.Value,
                name: "TradingAdvisor",
                tools: AgentTools.All);
        break;
    }

    // ── OpenAI (default) ─────────────────────────────────────────────
    case "openai":
    default:
    {
        if (string.IsNullOrEmpty(apiKey))
            throw new InvalidOperationException("OPENAI_API_KEY is required when AI_PROVIDER=openai");

        OpenAIClient openAIClient;
        if (!string.IsNullOrEmpty(endpoint))
        {
            var opts = new OpenAIClientOptions { Endpoint = new Uri(endpoint) };
            openAIClient = new OpenAIClient(new System.ClientModel.ApiKeyCredential(apiKey), opts);
        }
        else
        {
            openAIClient = new OpenAIClient(apiKey);
        }

        agent = openAIClient
            .GetChatClient(model)
            .AsIChatClient()
            .AsAIAgent(
                instructions: AgentInstructions.Value,
                name: "TradingAdvisor",
                tools: AgentTools.All);
        break;
    }
}

builder.Services.AddSingleton(agent);

Console.WriteLine($"AI provider : {provider}");
Console.WriteLine($"AI endpoint : {(string.IsNullOrEmpty(endpoint) ? "(default)" : endpoint)}");
Console.WriteLine($"AI model    : {model}");

// --- Agent guardrails (Agent Governance Toolkit sidecar) --------------------
//
// When the agentGuardrails Radius resource is enabled, app.bicep injects the
// AGT sidecar into the agent pod and surfaces the env vars below. We use the
// sidecar's /api/v1/detect/injection endpoint to scan inbound prompts BEFORE
// they reach the model. Calls are no-ops when GOVERNANCE_ENABLED is unset, so
// the same binary runs in portfolios where guardrails are off.
var guardrailsEnabled = string.Equals(
    Environment.GetEnvironmentVariable("GOVERNANCE_ENABLED"),
    "true",
    StringComparison.OrdinalIgnoreCase);
var guardrailsProxy = Environment.GetEnvironmentVariable("GOVERNANCE_PROXY")
    ?? Environment.GetEnvironmentVariable("GOVERNANCE_API")
    ?? "http://localhost:8081";
var guardrailsMode  = (Environment.GetEnvironmentVariable("GOVERNANCE_MODE") ?? "enforce").ToLowerInvariant();

builder.Services.AddHttpClient("guardrails", client =>
{
    client.BaseAddress = new Uri(guardrailsProxy);
    // Keep the budget tight — the sidecar is on localhost; anything slower than
    // a few hundred ms means the sidecar is unhealthy and we should fail open
    // (or closed, depending on mode) rather than stall the advice request.
    client.Timeout = TimeSpan.FromSeconds(2);
});

Console.WriteLine($"Guardrails  : {(guardrailsEnabled ? $"enabled (mode={guardrailsMode}, proxy={guardrailsProxy})" : "disabled")}");

// --- HTTP pipeline -----------------------------------------------------------

var app = builder.Build();
app.UseCors();

// POST /advice  –  { "question": "..." }  →  { "answer": "..." }
app.MapPost("/advice", async (AdviceRequest req, AIAgent agent, IHttpClientFactory httpClientFactory) =>
{
    using var activity = AiAgentTelemetry.ActivitySource.StartActivity("ai.generate_advice", ActivityKind.Internal);
    var question = req.Question?.Trim();
    if (string.IsNullOrEmpty(question))
    {
        AiAgentTelemetry.AdviceRequests.Add(1,
            new KeyValuePair<string, object?>("status", "bad_request"));
        return Results.BadRequest(new { answer = "Please provide a question." });
    }

    if (guardrailsEnabled)
    {
        var verdict = await AgentGuardrails.DetectInjectionAsync(
            httpClientFactory.CreateClient("guardrails"),
            question,
            guardrailsMode);

        if (verdict.Blocked)
        {
            activity?.SetTag("guardrails.blocked", true);
            activity?.SetTag("guardrails.reason", verdict.Reason);
            AiAgentTelemetry.AdviceRequests.Add(1,
                new KeyValuePair<string, object?>("status", "blocked"));
            return Results.Json(
                new { answer = $"Request was blocked by agent guardrails: {verdict.Reason}" },
                statusCode: StatusCodes.Status422UnprocessableEntity);
        }
    }

    try
    {
        var response = await agent.RunAsync(question);
        AiAgentTelemetry.AdviceRequests.Add(1,
            new KeyValuePair<string, object?>("status", "ok"));
        return Results.Ok(new { answer = response.Text ?? "I wasn't able to generate advice at this time." });
    }
    catch (Exception ex)
    {
        Console.Error.WriteLine($"Agent error: {ex}");
        AiAgentTelemetry.AdviceRequests.Add(1,
            new KeyValuePair<string, object?>("status", "error"));
        return Results.Json(
            new { answer = "Sorry, I'm unable to reach the AI model right now. Please check the configuration and try again." },
            statusCode: 502);
    }
});

// GET /health
app.MapGet("/health", () => Results.Ok(new { status = "ok", provider, model, endpoint, guardrails = guardrailsEnabled }));

app.Run();

// ---------------------------------------------------------------------------
// Request / response models
// ---------------------------------------------------------------------------
record AdviceRequest(string? Question);

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------
static class AgentInstructions
{
    public const string Value =
        """
        You are a professional trading advisor AI agent. You provide concise,
        actionable trading advice. You cover risk management, entry/exit strategies,
        position sizing, and market analysis. Always remind users that your advice is
        for educational purposes only and not a substitute for professional financial
        advice. Keep answers concise but informative (2-4 sentences).
        When asked about position sizing, use the calculate_position_size tool.
        When asked about market sentiment, use the get_market_sentiment tool.
        """;
}

static class AiAgentTelemetry
{
    public const string ActivitySourceName = "portable-apps.ai-agent";
    public const string MeterName = "portable-apps.ai-agent.metrics";

    public static readonly ActivitySource ActivitySource = new(ActivitySourceName);
    private static readonly Meter Meter = new(MeterName);

    public static readonly Counter<long> AdviceRequests = Meter.CreateCounter<long>(
        "ai_agent_advice_requests_total",
        description: "Total number of AI advice requests");

    public static readonly Counter<long> GuardrailsChecks = Meter.CreateCounter<long>(
        "ai_agent_guardrails_checks_total",
        description: "Total number of AGT sidecar prompt-injection checks (labels: result=allowed|blocked|audit|error)");
}

// ---------------------------------------------------------------------------
// Agent guardrails — thin HTTP client for the AGT sidecar's
// /api/v1/detect/injection endpoint. Documented at:
//   https://microsoft.github.io/agent-governance-toolkit/deployment/openclaw-sidecar/
//
// Failure policy:
//   • enforce  : sidecar unreachable / errored → BLOCK (fail closed).
//                A detected injection → BLOCK.
//   • audit    : sidecar unreachable / errored → ALLOW (fail open) and log.
//                A detected injection is logged but ALLOWED through.
//   • dryrun   : no API call is made; always ALLOW. Used to verify wiring
//                without exercising the sidecar.
// ---------------------------------------------------------------------------
static class AgentGuardrails
{
    public record Verdict(bool Blocked, string? Reason);

    public static async Task<Verdict> DetectInjectionAsync(HttpClient client, string text, string mode)
    {
        if (string.Equals(mode, "dryrun", StringComparison.OrdinalIgnoreCase))
        {
            AiAgentTelemetry.GuardrailsChecks.Add(1,
                new KeyValuePair<string, object?>("result", "dryrun"));
            return new Verdict(false, null);
        }

        InjectionResponse? result;
        try
        {
            var resp = await client.PostAsJsonAsync("/api/v1/detect/injection", new InjectionRequest
            {
                Text = text,
                Source = "user_input",
                Sensitivity = "balanced"
            });
            resp.EnsureSuccessStatusCode();
            result = await resp.Content.ReadFromJsonAsync<InjectionResponse>();
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"Guardrails sidecar error: {ex.Message}");
            AiAgentTelemetry.GuardrailsChecks.Add(1,
                new KeyValuePair<string, object?>("result", "error"));
            // Fail closed in enforce; fail open in audit.
            return string.Equals(mode, "audit", StringComparison.OrdinalIgnoreCase)
                ? new Verdict(false, null)
                : new Verdict(true, "guardrails sidecar unavailable");
        }

        if (result is { IsInjection: true })
        {
            if (string.Equals(mode, "audit", StringComparison.OrdinalIgnoreCase))
            {
                Console.Error.WriteLine(
                    $"[guardrails audit] prompt-injection detected (type={result.InjectionType}, confidence={result.Confidence:F2}) \u2014 allowing through per mode=audit");
                AiAgentTelemetry.GuardrailsChecks.Add(1,
                    new KeyValuePair<string, object?>("result", "audit"));
                return new Verdict(false, null);
            }

            var reason = string.IsNullOrEmpty(result.InjectionType)
                ? "prompt injection detected"
                : $"prompt injection detected ({result.InjectionType})";
            AiAgentTelemetry.GuardrailsChecks.Add(1,
                new KeyValuePair<string, object?>("result", "blocked"));
            return new Verdict(true, reason);
        }

        AiAgentTelemetry.GuardrailsChecks.Add(1,
            new KeyValuePair<string, object?>("result", "allowed"));
        return new Verdict(false, null);
    }

    private sealed class InjectionRequest
    {
        [JsonPropertyName("text")]        public string Text        { get; init; } = string.Empty;
        [JsonPropertyName("source")]      public string Source      { get; init; } = "user_input";
        [JsonPropertyName("sensitivity")] public string Sensitivity { get; init; } = "balanced";
    }

    private sealed class InjectionResponse
    {
        [JsonPropertyName("is_injection")]   public bool    IsInjection   { get; init; }
        [JsonPropertyName("threat_level")]   public string? ThreatLevel   { get; init; }
        [JsonPropertyName("injection_type")] public string? InjectionType { get; init; }
        [JsonPropertyName("confidence")]     public double  Confidence    { get; init; }
    }
}

// ---------------------------------------------------------------------------
// Agent tools – static methods decorated with [Description] for the framework
// ---------------------------------------------------------------------------
static class AgentTools
{
    public static AIFunction[] All =
    [
        AIFunctionFactory.Create(GetMarketSentiment),
        AIFunctionFactory.Create(CalculatePositionSize),
    ];

    [Description("Returns the current overall market sentiment (bullish, bearish, or neutral) with a short rationale.")]
    static string GetMarketSentiment(
        [Description("Optional ticker symbol to check sentiment for.")] string? symbol = null)
    {
        // Placeholder – replace with a real data-feed integration in production
        var sentiments = new[] { "bullish", "bearish", "neutral" };
        var sentiment = sentiments[Random.Shared.Next(sentiments.Length)];
        return JsonSerializer.Serialize(new
        {
            sentiment,
            rationale = $"Market is currently {sentiment} based on recent price action and volume trends.",
            symbol = symbol ?? "general market"
        });
    }

    [Description("Calculates a recommended position size given account balance, risk percentage, entry price, and stop-loss price.")]
    static string CalculatePositionSize(
        [Description("Total account balance in USD.")] double accountBalance,
        [Description("Percentage of account to risk (e.g. 1 for 1%).")] double riskPct,
        [Description("Planned entry price.")] double entryPrice,
        [Description("Planned stop-loss price.")] double stopLossPrice)
    {
        var riskAmount = accountBalance * (riskPct / 100.0);
        var priceRisk  = Math.Abs(entryPrice - stopLossPrice);
        var shares     = priceRisk > 0 ? (int)(riskAmount / priceRisk) : 0;

        return JsonSerializer.Serialize(new
        {
            recommended_shares = shares,
            risk_amount_usd    = riskAmount.ToString("F2"),
            price_risk_per_share = priceRisk.ToString("F2")
        });
    }
}
