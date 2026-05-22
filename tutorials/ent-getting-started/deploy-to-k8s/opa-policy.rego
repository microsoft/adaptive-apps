# Demo Rego policy for tutorial Section 5.
# Replaces the chart's default-allow policy. Denies any request that carries
# the header `x-deny: true`; allows everything else.
package envoy.authz

import rego.v1

# Default-allow so missing/unknown inputs don't accidentally lock the app out.
default allow := true

# Explicitly deny when the opt-in header is present.
# Header names from Envoy are lowercased.
allow := false if {
	input.attributes.request.http.headers["x-deny"] == "true"
}
