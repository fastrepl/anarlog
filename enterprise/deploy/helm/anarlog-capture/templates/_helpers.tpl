{{- define "anarlog-capture.fullname" -}}
{{- default .Chart.Name .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- define "anarlog-capture.secretName" -}}
{{- .Values.workspaceTokens.existingSecret -}}
{{- end -}}
{{- define "anarlog-capture.licenseSecretName" -}}
{{- .Values.license.existingSecret -}}
{{- end -}}
