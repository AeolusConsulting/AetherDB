{{- define "aetherdb.fullname" -}}
{{ .Release.Name }}-{{ .Chart.Name }}
{{- end }}

{{- define "aetherdb.name" -}}
{{ .Chart.Name }}
{{- end }}

{{- define "aetherdb.labels" -}}
app.kubernetes.io/name: {{ include "aetherdb.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version }}
{{- end }}

{{- define "aetherdb.selectorLabels" -}}
app.kubernetes.io/name: {{ include "aetherdb.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
