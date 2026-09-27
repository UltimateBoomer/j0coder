# Locoder on Kubernetes

This is the staging and production deployment path. The Helm 3 chart installs the Locoder control plane and a dedicated namespace for short-lived gVisor sandbox Pods. It does not install PostgreSQL, Valkey, gVisor, an ingress controller, cert-manager, or an external-secrets operator. For the default local development path, use [Podman Compose](../../../README.md#develop-locally).

For a disposable Fedora development cluster using a Lima-managed QEMU VM, kubeadm, containerd, and gVisor, see [Local Kubernetes on Fedora](../../../docs/local-kubernetes-fedora.md). Podman is used only to build and export the local images.

## Prerequisites

- Kubernetes 1.27 or newer and a CNI that enforces NetworkPolicy.
- A working gVisor RuntimeClass. Set `sandbox.runtimeClass` to its exact name. Worker and editor preflight creates a probe Pod and requires `dmesg` to identify gVisor; there is no runtime fallback.
- External PostgreSQL and Valkey endpoints in existing Secrets. The default keys are `DATABASE_URL` and `VALKEY_URL`.
- Immutable application and toolchain image digests. The same `deploy/App.Containerfile` builds the application image for Podman and Kubernetes; Kubernetes selects its sandbox backend through `SANDBOX_BACKEND=kubernetes`.
- Egress CIDRs for the Kubernetes API, DNS, PostgreSQL, Valkey, ingress path, and optional catalog SSH destination appropriate to the cluster.
- A TLS ingress and public HTTPS origin for browser access.

The published release workflow builds `ghcr.io/ultimateboomer/locoder-app` and `ghcr.io/ultimateboomer/locoder-toolchain` with the GitHub release tag, and reports the digests in the workflow summary. Set `images.app.digest` and `images.toolchain.digest` to those digests. GitHub initially makes new GHCR packages private. For private packages, create the application and sandbox namespaces before installing the chart, then create an image pull Secret in each namespace. Set `imagePullSecrets` for the app and `sandbox.imagePullSecrets` for dynamically created toolchain Pods. Both values contain Secret name references, for example `[{name: ghcr-pull}]`. Alternatively, make both packages public in GHCR before deploying without pull credentials.

Create a staging or production values file from `values.yaml`. Set `publicOrigin`, `ingress.host`, `ingress.tlsSecret`, both image repositories and digests, `sandbox.runtimeClass`, the existing Secret references, and the network policy destinations for your cluster. Use separate values and credentials for each environment. Install after the external services, Secrets, RuntimeClass, and ingress are ready:

```sh
helm upgrade --install locoder deploy/helm/locoder \
  --namespace locoder --create-namespace -f production-values.yaml
```

The chart does not create a default administrator. Once the API Deployment is ready, create the initial administrator by supplying its password on stdin:

```bash
read -rsp 'New admin password (12+ characters): ' locoder_password; echo
printf '%s\n' "$locoder_password" | kubectl -n locoder exec -i deployment/locoder-api -- api bootstrap-admin admin
unset locoder_password
```

In fish:

```fish
read --silent --prompt-str 'New admin password (12+ characters): ' locoder_password
echo
printf '%s\n' "$locoder_password" | kubectl -n locoder exec -i deployment/locoder-api -- api bootstrap-admin admin
set -e locoder_password
```

Sign in as `admin` at the configured public origin. Run the bootstrap command only for a new database; an existing username causes it to fail. The command runs inside an API Pod with the chart's database connection. If you use a different Helm release name or namespace, adjust `deployment/locoder-api` and `-n locoder` accordingly.

The pre-install/pre-upgrade Job runs `api migrate`; API replicas set `MIGRATE_ON_START=false`. Database and Valkey Secrets are referenced, never copied into chart-managed objects. Worker and editor service accounts can only manage Pods and Pod attachments in the sandbox namespace. Sandbox Pods do not mount service-account tokens and are isolated by a default-deny policy.

## Capacity and scheduling

Execution capacity is approximately `replicas.worker × workerConcurrency`, plus compilation bursts and active editor sessions. Every test case gets a fresh Pod; one stateful trace remains within one Pod. Larger installations should reserve gVisor nodes for execution with `sandbox.nodeSelector` and `sandbox.tolerations`; controller placement is configured separately under `scheduling`. API and editor CPU HPAs are optional; worker autoscaling is deliberately left to an external queue-aware system such as KEDA.

## Lifecycle and failures

Source, case input, compiled artifacts, and results travel only through the Kubernetes attach stream. They are never put in Secrets, ConfigMaps, labels, annotations, or shared volumes. Pods are deleted after completion, cancellation, timeout, disconnect, or shutdown. Expiry labels plus the periodic worker sweeper remove terminal or orphaned Pods after controller failure.

If the RuntimeClass is missing or does not identify as gVisor, the control-plane resources can install, but workers and editors fail preflight and remain unavailable. Inspect their logs and the probe Pod events; do not change the RuntimeClass to `runc` as a workaround.

## Upgrade and rollback

Run `helm upgrade` with new immutable digests. The migration hook finishes before Deployments roll. API and editor PodDisruptionBudgets retain availability, while an editor WebSocket naturally remains on its chosen replica until that Pod terminates. The catalog controller uses `Recreate` and one replica.

Use `helm rollback locoder REVISION` for application rollback. Database migrations must remain backward-compatible with the prior application revision; Helm does not reverse schema migrations. Confirm worker queue compatibility and gVisor capacity before rollback.
