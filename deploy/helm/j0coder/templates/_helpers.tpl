{{- define "j0coder.name" -}}{{ .Release.Name }}{{- end }}
{{- define "j0coder.labels" -}}app.kubernetes.io/name: j0coder
app.kubernetes.io/instance: {{ .Release.Name }}{{- end }}
{{- define "j0coder.sandboxNamespace" -}}{{ default (printf "%s-sandbox" .Release.Name) .Values.sandbox.namespace }}{{- end }}
{{- define "j0coder.appImage" -}}{{ .Values.images.app.repository }}@{{ .Values.images.app.digest }}{{- end }}
{{- define "j0coder.toolchainImage" -}}{{ .Values.images.toolchain.repository }}@{{ .Values.images.toolchain.digest }}{{- end }}
{{- define "j0coder.containerSecurity" -}}
allowPrivilegeEscalation: false
capabilities: {drop: ["ALL"]}
readOnlyRootFilesystem: true
runAsNonRoot: true
runAsUser: 65534
runAsGroup: 65534
seccompProfile: {type: RuntimeDefault}
{{- end }}
{{- define "j0coder.podSecurity" -}}
runAsNonRoot: true
runAsUser: 65534
runAsGroup: 65534
fsGroup: 65534
seccompProfile: {type: RuntimeDefault}
{{- end }}
