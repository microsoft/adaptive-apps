const { NodeSDK } = require("@opentelemetry/sdk-node");
const { getNodeAutoInstrumentations } = require("@opentelemetry/auto-instrumentations-node");
const { OTLPTraceExporter } = require("@opentelemetry/exporter-trace-otlp-http");
const { OTLPMetricExporter } = require("@opentelemetry/exporter-metrics-otlp-http");
const { PeriodicExportingMetricReader } = require("@opentelemetry/sdk-metrics");

const traceExporter = new OTLPTraceExporter();
const metricExporter = new OTLPMetricExporter();

const sdk = new NodeSDK({
  traceExporter,
  metricReader: new PeriodicExportingMetricReader({
    exporter: metricExporter,
    exportIntervalMillis: Number.parseInt(process.env.OTEL_METRIC_EXPORT_INTERVAL, 10) || 10000,
  }),
  instrumentations: [getNodeAutoInstrumentations()],
});

try {
  const startup = sdk.start();
  if (startup && typeof startup.then === "function") {
    startup.catch((err) => {
      console.error("OpenTelemetry startup error", err);
    });
  }
} catch (err) {
  console.error("OpenTelemetry startup error", err);
}

process.on("SIGTERM", () => {
  try {
    const shutdown = sdk.shutdown();
    if (shutdown && typeof shutdown.then === "function") {
      shutdown.catch((err) => {
        console.error("OpenTelemetry shutdown error", err);
      });
    }
  } catch (err) {
    console.error("OpenTelemetry shutdown error", err);
  }
});
