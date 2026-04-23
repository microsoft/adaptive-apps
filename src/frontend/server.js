require("./telemetry");

const express = require("express");
const path = require("path");
const session = require("express-session");
const passport = require("passport");
const LocalStrategy = require("passport-local").Strategy;
const { createProxyMiddleware } = require("http-proxy-middleware");
const http = require("http");
const { metrics } = require("@opentelemetry/api");

const app = express();
const port = process.env.PORT || 3000;

const meter = metrics.getMeter("portable-apps.frontend.metrics");
const requestCount = meter.createCounter("frontend_http_requests_total", {
  description: "Total number of HTTP requests handled by the frontend",
});
const requestDurationMs = meter.createHistogram("frontend_http_request_duration_ms", {
  unit: "ms",
  description: "HTTP request duration in milliseconds",
});

app.use((req, res, next) => {
  const started = process.hrtime.bigint();
  res.on("finish", () => {
    const elapsedNs = process.hrtime.bigint() - started;
    const elapsedMs = Number(elapsedNs) / 1_000_000;
    const method = req.method;
    const route = req.route?.path || req.path;
    const statusCode = String(res.statusCode);

    requestCount.add(1, {
      method,
      route,
      status_code: statusCode,
    });

    requestDurationMs.record(elapsedMs, {
      method,
      route,
      status_code: statusCode,
    });
  });

  next();
});

// ---------------------------------------------------------------------------
// App config (unchanged)
// ---------------------------------------------------------------------------
const config = {
  backendUrl: process.env.BACKEND_URL || "http://localhost:5000",
  aiAgentUrl: process.env.AI_AGENT_URL || "http://localhost:7000",
  mqttWsUrl: process.env.MQTT_WS_URL || "ws://localhost:9001",
  mqttTopic: process.env.MQTT_TOPIC || "orders/new",
};

// ---------------------------------------------------------------------------
// Auth configuration
// ---------------------------------------------------------------------------
const AUTH_USERNAME = process.env.AUTH_USERNAME || "admin";
const AUTH_PASSWORD = process.env.AUTH_PASSWORD || "admin";
const SESSION_SECRET = process.env.SESSION_SECRET || "change-me-in-production";

// Generic OIDC settings (optional). Keep AAD_* fallbacks for compatibility.
const OIDC_ISSUER = process.env.OIDC_ISSUER || "";
const OIDC_AUTH_ENDPOINT = process.env.OIDC_AUTH_ENDPOINT || "";
const OIDC_BROWSER_AUTH_ENDPOINT = process.env.OIDC_BROWSER_AUTH_ENDPOINT || "";
const OIDC_TOKEN_ENDPOINT = process.env.OIDC_TOKEN_ENDPOINT || "";
const OIDC_USERINFO_ENDPOINT = process.env.OIDC_USERINFO_ENDPOINT || "";
const OIDC_CLIENT_ID = process.env.OIDC_CLIENT_ID || process.env.AAD_CLIENT_ID || "";
const OIDC_CLIENT_SECRET =
  process.env.OIDC_CLIENT_SECRET || process.env.AAD_CLIENT_SECRET || "";

// Public URL where users access the frontend, e.g. https://trading.example.com
// Used to build the OIDC callback URL when OIDC_REDIRECT_URI is not set.
const APP_BASE_URL = process.env.APP_BASE_URL || "";

// "common" = work/school + personal MS accounts
// "organizations" = work/school only
// "consumers" = personal MS accounts only
// or a specific tenant ID
const AAD_TENANT_ID = process.env.AAD_TENANT_ID || "common";

const redirectPath = "/auth/microsoft/callback";
const OIDC_REDIRECT_URI =
  process.env.OIDC_REDIRECT_URI ||
  process.env.AAD_REDIRECT_URI ||
  (APP_BASE_URL
    ? `${APP_BASE_URL.replace(/\/$/, "")}${redirectPath}`
    : `http://localhost:${port}${redirectPath}`);

const hasOidcCredentials = !!(OIDC_CLIENT_ID && OIDC_CLIENT_SECRET);
const defaultAuthEndpoint = `https://login.microsoftonline.com/${AAD_TENANT_ID}/oauth2/v2.0/authorize`;
const defaultTokenEndpoint = `https://login.microsoftonline.com/${AAD_TENANT_ID}/oauth2/v2.0/token`;
const defaultUserInfoEndpoint = "https://graph.microsoft.com/oidc/userinfo";
const effectiveIssuer = OIDC_ISSUER || `https://login.microsoftonline.com/${AAD_TENANT_ID}/v2.0`;
const effectiveAuthEndpoint = OIDC_AUTH_ENDPOINT || defaultAuthEndpoint;
// Browser must be able to reach this URL; use override when issuer/auth endpoint is in-cluster only.
const effectiveBrowserAuthEndpoint = OIDC_BROWSER_AUTH_ENDPOINT || effectiveAuthEndpoint;
const effectiveTokenEndpoint = OIDC_TOKEN_ENDPOINT || defaultTokenEndpoint;
const effectiveUserInfoEndpoint = OIDC_USERINFO_ENDPOINT || defaultUserInfoEndpoint;
const hasOidcEndpoints = !!(
  effectiveIssuer &&
  effectiveBrowserAuthEndpoint &&
  effectiveTokenEndpoint &&
  effectiveUserInfoEndpoint
);
const microsoftEnabled = hasOidcCredentials && hasOidcEndpoints;

// ---------------------------------------------------------------------------
// Session + Passport setup
// ---------------------------------------------------------------------------
app.use(express.urlencoded({ extended: false }));

app.use(
  session({
    secret: SESSION_SECRET,
    resave: false,
    saveUninitialized: false,
    cookie: { maxAge: 8 * 60 * 60 * 1000 }, // 8 hours
  })
);

app.use(passport.initialize());
app.use(passport.session());

passport.serializeUser((user, done) => done(null, user));
passport.deserializeUser((user, done) => done(null, user));

// --- Local (password) strategy ---
passport.use(
  new LocalStrategy((username, password, done) => {
    if (username === AUTH_USERNAME && password === AUTH_PASSWORD) {
      return done(null, { id: username, displayName: username, provider: "local" });
    }
    return done(null, false, { message: "Invalid username or password" });
  })
);

// --- OIDC strategy (loaded only when configured) ---
if (microsoftEnabled) {
  const OpenIDConnectStrategy = require("passport-openidconnect").Strategy;

  const oidcOptions = {
    issuer: effectiveIssuer,
    authorizationURL: effectiveBrowserAuthEndpoint,
    tokenURL: effectiveTokenEndpoint,
    userInfoURL: effectiveUserInfoEndpoint,
    clientID: OIDC_CLIENT_ID,
    clientSecret: OIDC_CLIENT_SECRET,
    callbackURL: OIDC_REDIRECT_URI,
    scope: ["openid", "profile", "email"].join(" "),
    passReqToCallback: false,
  };

  passport.use(
    "microsoft",
    new OpenIDConnectStrategy(
      oidcOptions,
      (iss, profile, done) => {
        const user = {
          id: profile.id || profile.oid || profile.sub || profile._json?.sub,
          displayName:
            profile.displayName ||
            profile.name ||
            profile._json?.name ||
            profile._json?.preferred_username ||
            profile.id,
          email:
            profile.emails?.[0]?.value ||
            profile._json?.email ||
            profile._json?.preferred_username ||
            profile._json?.upn ||
            "",
          provider: "microsoft",
        };
        return done(null, user);
      }
    )
  );
}

if (!microsoftEnabled) {
  const missing = [];
  if (!OIDC_CLIENT_ID) missing.push("OIDC_CLIENT_ID");
  if (!OIDC_CLIENT_SECRET) missing.push("OIDC_CLIENT_SECRET");
  if (!effectiveIssuer) missing.push("OIDC_ISSUER");
  if (!effectiveBrowserAuthEndpoint) missing.push("OIDC_AUTH_ENDPOINT or OIDC_BROWSER_AUTH_ENDPOINT");
  if (!effectiveTokenEndpoint) missing.push("OIDC_TOKEN_ENDPOINT");
  if (!effectiveUserInfoEndpoint) missing.push("OIDC_USERINFO_ENDPOINT");
  if (missing.length > 0) {
    console.warn(`OIDC disabled: missing config ${missing.join(", ")}`);
  }
}

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------
function ensureAuth(req, res, next) {
  if (req.isAuthenticated()) return next();
  // Allow the login page assets through
  return res.redirect("/login.html");
}

// ---------------------------------------------------------------------------
// Public routes (no auth required)
// ---------------------------------------------------------------------------

// Auth providers endpoint – tells the login page which methods are available
app.get("/auth/providers", (req, res) => {
  res.json({ local: true, microsoft: microsoftEnabled });
});

// Login page assets served without auth
app.get("/login.html", (req, res) => {
  if (req.isAuthenticated()) return res.redirect("/");
  res.sendFile(path.join(__dirname, "public", "login.html"));
});

// Local login
app.post(
  "/auth/login",
  passport.authenticate("local", {
    failureRedirect: "/login.html?error=Invalid+username+or+password",
  }),
  (req, res) => res.redirect("/")
);

// Microsoft login
if (microsoftEnabled) {
  const completeOidcLogin = (req, res, next) => {
    passport.authenticate("microsoft", (err, user, info) => {
      if (err) {
        console.error("OIDC callback error:", err);
        return res.redirect("/login.html?error=OIDC+sign-in+failed");
      }
      if (!user) {
        const reason = typeof info === "string"
          ? info
          : info?.message || "no user returned";
        console.warn("OIDC callback failed:", reason);
        return res.redirect("/login.html?error=OIDC+sign-in+failed");
      }

      req.logIn(user, (loginErr) => {
        if (loginErr) {
          console.error("OIDC session login error:", loginErr);
          return res.redirect("/login.html?error=OIDC+sign-in+failed");
        }
        return res.redirect("/");
      });
    })(req, res, next);
  };

  app.get(
    "/auth/microsoft",
    passport.authenticate("microsoft", { prompt: "select_account" })
  );

  // Most OIDC providers (including Keycloak default) return the code via GET.
  app.get("/auth/microsoft/callback", completeOidcLogin);

  // Keep POST callback for providers configured with response_mode=form_post.
  app.post("/auth/microsoft/callback", completeOidcLogin);
}

// Logout
app.get("/auth/logout", (req, res, next) => {
  req.logout((err) => {
    if (err) return next(err);
    req.session.destroy(() => {
      res.redirect("/login.html");
    });
  });
});

// ---------------------------------------------------------------------------
// Protected routes
// ---------------------------------------------------------------------------

// /config — browser only needs MQTT topic and info; URLs are now relative
app.get("/config", ensureAuth, (req, res) => {
  res.json({
    backendUrl: "",            // relative — proxied through this server at /api/*
    aiAgentUrl: "",            // relative — proxied through this server at /advice
    mqttWsUrl: `ws://${req.headers.host}/mqtt`,  // WebSocket proxied at /mqtt
    mqttTopic: config.mqttTopic,
  });
});

// Current user info (useful for the UI)
app.get("/auth/me", ensureAuth, (req, res) => {
  res.json({
    displayName: req.user.displayName,
    provider: req.user.provider,
  });
});

// ---------------------------------------------------------------------------
// Reverse proxies — all backend traffic flows through the frontend server
// so only the frontend port needs to be exposed.
// ---------------------------------------------------------------------------

// Proxy /api/* → backend service
const backendProxy = createProxyMiddleware({
  target: config.backendUrl,
  changeOrigin: true,
  pathFilter: '/api',
});
app.use(ensureAuth, backendProxy);

// Proxy /advice → ai-agent service
const aiProxy = createProxyMiddleware({
  target: config.aiAgentUrl,
  changeOrigin: true,
  pathFilter: '/advice',
});
app.use(ensureAuth, aiProxy);

// MQTT WebSocket proxy is set up on the HTTP server after listen (see bottom of file).
const mqttProxy = createProxyMiddleware({
  target: config.mqttWsUrl,
  changeOrigin: true,
  ws: true,
  pathFilter: '/mqtt',
  pathRewrite: { '^/mqtt': '' },
});
app.use(mqttProxy);

// Serve static files (protected)
app.use(ensureAuth, express.static(path.join(__dirname, "public")));

app.get("/{*path}", ensureAuth, (req, res) => {
  res.sendFile(path.join(__dirname, "public", "index.html"));
});

const server = http.createServer(app);

// Handle WebSocket upgrade for /mqtt path
server.on("upgrade", (req, socket, head) => {
  if (req.url && req.url.startsWith("/mqtt")) {
    mqttProxy.upgrade(req, socket, head);
  } else {
    socket.destroy();
  }
});

server.listen(port, () => {
  console.log(`Frontend running on port ${port}`);
  console.log(`  Local auth    : enabled (user: ${AUTH_USERNAME})`);
  console.log(`  OIDC issuer   : ${effectiveIssuer}`);
  console.log(`  OIDC auth URL : ${effectiveAuthEndpoint}`);
  if (OIDC_BROWSER_AUTH_ENDPOINT) {
    console.log(`  OIDC browser auth URL : ${effectiveBrowserAuthEndpoint}`);
  }
  console.log(`  OIDC token URL: ${effectiveTokenEndpoint}`);
  console.log(`  OIDC userinfo : ${effectiveUserInfoEndpoint}`);
  console.log(`  OIDC callback : ${OIDC_REDIRECT_URI}`);
  console.log(`  OIDC auth     : ${microsoftEnabled ? "enabled" : "disabled"}`);
  console.log(`  Backend proxy : ${config.backendUrl} → /api`);
  console.log(`  AI agent proxy: ${config.aiAgentUrl} → /advice`);
  console.log(`  MQTT WS proxy : ${config.mqttWsUrl} → /mqtt`);
});
