# Challenge 06 - Adapt Identity Services - Configure User Authentication

[< Previous Challenge](./Challenge-05.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-07.md)

## Challenge Metadata

- Difficulty Level: Intermediate to Advanced
- Estimated Time: 60-90 minutes
- Target Audience: Platform engineers and application engineers integrating identity across multiple environments
- Prerequisites:
	- Completion of [Challenge 05](./Challenge-05.md)
	- App deployed in both target environments with stable routing to the frontend
	- Operational Keycloak instance(s) reachable for each target environment
	- Access to at least one cloud identity provider path and one local or alternate identity path
- Learning Objectives:
	- Apply the identity-broker pattern so app authentication remains stable across environments
	- Configure a reusable OIDC client contract for the frontend
	- Integrate environment-specific upstream identity providers without rewriting the app model
	- Validate and troubleshoot identity flow boundaries from frontend to broker to upstream IdP

## Scenario

Security and compliance stakeholders require identity modernization for the trading application. The frontend experience must remain consistent while each environment aligns with its own identity authority and operational constraints.

Your team must deliver a brokered authentication approach where the application continues to use one OIDC integration pattern, while upstream identity differs per environment. In one environment, enterprise cloud identity is expected. In another, local or edge-compatible identity is required.

This matters because identity coupling can break portability even when infrastructure is portable. A successful outcome proves that environment-specific identity plumbing can change without forcing application rewrites.

## Challenge Goals

1. Establish a stable frontend OIDC client contract through Keycloak.
2. Configure different upstream identity integrations per environment while preserving the same app-facing authentication flow.
3. Demonstrate successful sign-in behavior in both environments and explain the portability boundary.

## Requirements and Constraints

- Keep the frontend OIDC contract stable across environments.
- Upstream identity integrations may differ by environment, but login flow ownership boundaries must be explicit.
- Environment-specific identity configuration must not require changes to core application model logic.
- Authentication validation must include both positive sign-in proof and targeted troubleshooting reasoning.
- If using a workshop stand-in for local identity, document the limitation clearly and still demonstrate the same architectural pattern.

## Tasks

### Task 1 - Define Identity Architecture and Ownership

Map the end-to-end authentication flow for both environments. Identify which settings are app-owned, which are broker-owned, and which are upstream provider-owned.

### Task 2 - Configure the Brokered OIDC Contract

Implement and validate the Keycloak OIDC client configuration the frontend will consume. Confirm the contract is reusable in both environments.

### Task 3 - Integrate Environment-Specific Upstream Identity

Configure upstream federation per environment (cloud enterprise identity for one target and local/alternate identity for the other). Ensure the broker can route authentication correctly.

### Task 4 - Validate Login Portability and Troubleshooting Model

Execute sign-in validation in both environments and produce a short diagnostic framework your team can use when authentication fails.

## Success Criteria

You are done when all of the following are true:

- The frontend uses a stable OIDC integration pattern in both environments.
- Broker configuration for the application client is present and functionally validated.
- Each environment can authenticate through its intended upstream identity path.
- The team can explain what changed between environments and what remained invariant.
- The team can isolate failures by layer (frontend client config, broker config, upstream provider configuration, user assignment/connectivity).

## Hints (Progressive Disclosure)

### Task 1 Hints

- Hint 1: Draw the auth sequence before changing any settings.
- Hint 2: Keep app integration concerns separate from federation concerns.
- Hint 3: If ownership is unclear, label each configuration value as app, broker, or upstream.

### Task 2 Hints

- Hint 1: OIDC client correctness is foundational; validate it before federation debugging.
- Hint 2: Redirect and issuer-related mismatches often masquerade as upstream login failures.
- Hint 3: Reuse one client contract deliberately and avoid per-environment app-client drift unless justified.

### Task 3 Hints

- Hint 1: Treat each environment's upstream provider as an implementation detail behind the broker.
- Hint 2: Validate provider-side assignments/mappings in addition to broker-side configuration.
- Hint 3: For LDAP-style paths, verify connectivity and trust settings before user sync assumptions.

### Task 4 Hints

- Hint 1: Validate end-to-end with real users or test identities for each environment path.
- Hint 2: Capture both successful flow evidence and one failed-case troubleshooting path.
- Hint 3: Use a layered checklist so future incidents can be triaged quickly.

## Learning Resources

- Identity and OIDC foundations:
	- [OpenID Connect core concepts](https://openid.net/developers/how-connect-works/)
	- [Keycloak identity brokering](https://www.keycloak.org/docs/latest/server_admin/#_identity_broker)
- Environment-specific integration references:
	- [Microsoft Entra SAML-based app federation](https://learn.microsoft.com/entra/identity/enterprise-apps/add-application-portal-setup-sso)
	- [Keycloak LDAP user federation](https://www.keycloak.org/docs/latest/server_admin/#_ldap)
- Workshop context assets:
	- [Authentication documentation folder](../../docs/authentication/README.md)
	- [Azure sample architecture image](../sampleazure.png)
	- [Local sample architecture image](../samplelocal.png)

## Optional Stretch

If you finish early, compare token claims across both environment login paths and document how claim normalization could simplify app-level authorization logic.
