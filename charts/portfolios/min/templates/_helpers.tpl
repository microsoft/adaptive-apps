{{/* Expand the name of the chart. */}}
{{- define "min.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/* Create a default fully qualified app name.

     Resource names are always `<release-name>` (optionally overridden via
     fullnameOverride). The chart name is intentionally not included so that
     when `min` is consumed as a subchart of `core`, `core-ai`, etc., resources
     are named `<release>-keycloak-*` instead of `<release>-min-keycloak-*`. */}}
{{- define "min.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{/* Shared labels */}}
{{- define "min.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | quote }}
app.kubernetes.io/name: {{ include "min.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end -}}

{{/* Selector labels */}}
{{- define "min.selectorLabels" -}}
app.kubernetes.io/name: {{ include "min.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* Keycloak service name */}}
{{- define "min.keycloak.serviceName" -}}
{{ include "min.fullname" . }}-keycloak
{{- end -}}

{{/* Keycloak discovery service name */}}
{{- define "min.keycloak.discoveryServiceName" -}}
{{ printf "%s-discovery" (include "min.keycloak.serviceName" .) }}
{{- end -}}

{{/* PostgreSQL resource base name */}}
{{- define "min.keycloak.postgresqlName" -}}
{{ printf "%s-postgresql" (include "min.keycloak.serviceName" .) }}
{{- end -}}

{{/*
  Effective OIDC values dictionary.

  Returns YAML for `mergeOverwrite (local) (global)` so that
  `--set global.oidc.<x>=...` at any chart level (min directly or via core,
  core-ai, ent, ent-ai) overrides the local default. Consumers should pipe
  through `fromYaml`:

      {{- $oidc := include "min.oidc" . | fromYaml -}}
      {{- $oidc.clientId -}}
*/}}
{{- define "min.oidc" -}}
{{- $g := default (dict) .Values.global -}}
{{- $go := default (dict) (get $g "oidc") -}}
{{- mergeOverwrite (deepCopy .Values.oidc) $go | toYaml -}}
{{- end -}}

{{/*
  Effective Keycloak enable flag as a string ("true"/"false").

  Honors `--set global.components.keycloak.enabled=...` when set; otherwise
  falls back to local `components.keycloak.enabled`. Use with `eq`:

      {{- if eq (include "min.keycloakEnabled" .) "true" }}
*/}}
{{- define "min.keycloakEnabled" -}}
{{- $g := default (dict) .Values.global -}}
{{- $gc := default (dict) (get $g "components") -}}
{{- $gk := default (dict) (get $gc "keycloak") -}}
{{- if hasKey $gk "enabled" -}}
{{- $gk.enabled -}}
{{- else -}}
{{- .Values.components.keycloak.enabled -}}
{{- end -}}
{{- end -}}
