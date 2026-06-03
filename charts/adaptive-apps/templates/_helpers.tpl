{{/*
  adaptive-apps unified chart helpers.

  Canonical helpers are under the `adaptive.*` namespace. `min.*`, `core.*`,
  `ent.*` aliases are exposed below so identity/mesh/governance templates
  can be referenced under either name.
*/}}

{{/* Chart name */}}
{{- define "adaptive.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
  Fully qualified release name. Resource names are `<release-name>` (optionally
  overridden by fullnameOverride) so a release named e.g. `core` produces
  `core-keycloak-*`, matching the legacy per-portfolio chart output.
*/}}
{{- define "adaptive.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{/* Shared labels */}}
{{- define "adaptive.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | quote }}
app.kubernetes.io/name: {{ include "adaptive.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: adaptive-apps
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end -}}

{{/* Selector labels */}}
{{- define "adaptive.selectorLabels" -}}
app.kubernetes.io/name: {{ include "adaptive.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* Keycloak service name */}}
{{- define "adaptive.keycloak.serviceName" -}}
{{ include "adaptive.fullname" . }}-keycloak
{{- end -}}

{{/* Keycloak discovery service name */}}
{{- define "adaptive.keycloak.discoveryServiceName" -}}
{{ printf "%s-discovery" (include "adaptive.keycloak.serviceName" .) }}
{{- end -}}

{{/* PostgreSQL resource base name */}}
{{- define "adaptive.keycloak.postgresqlName" -}}
{{ printf "%s-postgresql" (include "adaptive.keycloak.serviceName" .) }}
{{- end -}}

{{/*
  Effective OIDC values dictionary. Honors `--set global.oidc.<x>=...` if
  callers want to override at install time. Consumers should pipe through
  fromYaml:

      {{- $oidc := include "adaptive.oidc" . | fromYaml -}}
*/}}
{{- define "adaptive.oidc" -}}
{{- $g := default (dict) .Values.global -}}
{{- $go := default (dict) (get $g "oidc") -}}
{{- mergeOverwrite (deepCopy .Values.oidc) $go | toYaml -}}
{{- end -}}

{{/*
  Feature gate: identity stack (Keycloak + PostgreSQL).
  Returns "true"/"false".
*/}}
{{- define "adaptive.feature.identity" -}}
{{- if hasKey (default (dict) .Values.features) "identity" -}}
{{- (default (dict) .Values.features.identity).enabled | default false -}}
{{- else -}}
true
{{- end -}}
{{- end -}}

{{/*
  Alias used by identity templates. Maps to the new feature flag.
*/}}
{{- define "adaptive.keycloakEnabled" -}}
{{ include "adaptive.feature.identity" . }}
{{- end -}}

{{/* -------------------------------------------------------------------------
     Legacy shims so unmodified templates from min/, core/, ent/ still resolve.
   ------------------------------------------------------------------------- */}}

{{- define "min.name" -}}{{ include "adaptive.name" . }}{{- end -}}
{{- define "min.fullname" -}}{{ include "adaptive.fullname" . }}{{- end -}}
{{- define "min.labels" -}}{{ include "adaptive.labels" . }}{{- end -}}
{{- define "min.selectorLabels" -}}{{ include "adaptive.selectorLabels" . }}{{- end -}}
{{- define "min.keycloak.serviceName" -}}{{ include "adaptive.keycloak.serviceName" . }}{{- end -}}
{{- define "min.keycloak.discoveryServiceName" -}}{{ include "adaptive.keycloak.discoveryServiceName" . }}{{- end -}}
{{- define "min.keycloak.postgresqlName" -}}{{ include "adaptive.keycloak.postgresqlName" . }}{{- end -}}
{{- define "min.oidc" -}}{{ include "adaptive.oidc" . }}{{- end -}}
{{- define "min.keycloakEnabled" -}}{{ include "adaptive.keycloakEnabled" . }}{{- end -}}

{{- define "core.name" -}}{{ include "adaptive.name" . }}{{- end -}}
{{- define "core.labels" -}}{{ include "adaptive.labels" . }}
app.kubernetes.io/component: mesh
{{- end -}}

{{- define "ent.name" -}}{{ include "adaptive.name" . }}{{- end -}}
{{- define "ent.labels" -}}{{ include "adaptive.labels" . }}
app.kubernetes.io/component: governance
{{- end -}}
