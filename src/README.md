# Simplified Trading App

Multi-component demo app with:

- `frontend` (Node.js + static UI)
- `backend` (C# minimal API + MQTT order processing)
- `ai-agent` (C# AI assistant API)
- `mosquitto` MQTT broker
- `postgres` data store
- `otel-collector` OpenTelemetry collector
- `zipkin` trace viewer
- `prometheus` metrics viewer

## Run with Docker Compose

From the `src` folder:

```
docker compose up --build
```

Then open `http://localhost:3000`.

Observability endpoints:

- Zipkin traces: `http://localhost:9411`
- Prometheus metrics: `http://localhost:9090`

## Data Flow

- The frontend publishes orders to MQTT topic `orders/new` over WebSockets.
- The backend subscribes and stores orders/trades/positions in PostgreSQL.
- The frontend queries the backend for symbols, orders, trades, positions.
- The AI agent provides short advisory responses to user questions.
- Frontend, backend, and ai-agent all emit OpenTelemetry traces and metrics.
- Traces flow through the collector to Zipkin; metrics flow through the collector to Prometheus.
