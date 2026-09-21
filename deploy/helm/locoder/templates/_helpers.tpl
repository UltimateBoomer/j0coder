{{- define "locoder.name" -}}{{ .Release.Name }}{{- end }}
{{- define "locoder.labels" -}}app.kubernetes.io/name: locoder
app.kubernetes.io/instance: {{ .Release.Name }}{{- end }}
{{- define "locoder.sandboxNamespace" -}}{{ default (printf "%s-sandbox" .Release.Name) .Values.sandbox.namespace }}{{- end }}
{{- define "locoder.appImage" -}}{{ .Values.images.app.repository }}@{{ .Values.images.app.digest }}{{- end }}
{{- define "locoder.toolchainImage" -}}{{ .Values.images.toolchain.repository }}@{{ .Values.images.toolchain.digest }}{{- end }}
{{- define "locoder.containerSecurity" -}}
allowPrivilegeEscalation: false
capabilities: {drop: ["ALL"]}
readOnlyRootFilesystem: true
runAsNonRoot: true
runAsUser: 65534
runAsGroup: 65534
seccompProfile: {type: RuntimeDefault}
{{- end }}
{{- define "locoder.podSecurity" -}}
runAsNonRoot: true
runAsUser: 65534
runAsGroup: 65534
fsGroup: 65534
seccompProfile: {type: RuntimeDefault}
{{- end }}
