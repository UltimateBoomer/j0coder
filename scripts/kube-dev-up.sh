#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
profile=${LIMA_INSTANCE:-locoder}
cpus=${LIMA_CPUS:-6}
memory=${LIMA_MEMORY:-12}
disk=${LIMA_DISK_SIZE:-40}
app_image=${KUBERNETES_APP_IMAGE:-localhost/locoder-app-kubernetes:1}
toolchain_image=${TOOLCHAIN_IMAGE:-localhost/locoder-toolchain:1}

case "$(uname -m)" in
    x86_64) qemu_command=qemu-system-x86_64 ;;
    aarch64) qemu_command=qemu-system-aarch64 ;;
    *) echo "Lima's QEMU workflow is unsupported on $(uname -m)" >&2; exit 1 ;;
esac

for command in limactl kubectl helm podman python3 curl sha512sum bzip2 tar awk "$qemu_command"; do
    command -v "$command" >/dev/null || { echo "missing required command: $command" >&2; exit 1; }
done
test -f "$root/.env" || { echo "missing .env; run make configure" >&2; exit 1; }
limactl validate "$root/deploy/local-kubernetes/lima.yaml" >/dev/null

if [[ ! -e /dev/kvm ]]; then
    echo "/dev/kvm is unavailable; enable AMD-V or Intel VT-x in firmware and load the kvm_amd or kvm_intel module" >&2
    exit 1
fi
if [[ ! -r /dev/kvm || ! -w /dev/kvm ]]; then
    echo "/dev/kvm is not accessible to $(id -un); add the user to the kvm group, then log out and back in" >&2
    exit 1
fi
if [[ "$(uname -m)" == x86_64 ]] && ! grep -Eqw '(svm|vmx)' /proc/cpuinfo; then
    echo "hardware virtualization flags (svm or vmx) are not visible to the host" >&2
    exit 1
fi

if limactl list --quiet | grep -Fxq "$profile"; then
    vm_type=$(limactl list "$profile" --format '{{.VMType}}')
    if [[ "$vm_type" != qemu ]]; then
        echo "Lima instance $profile uses VM type $vm_type; this workflow requires qemu." >&2
        echo "Delete the disposable instance first: make kube-dev-down" >&2
        exit 1
    fi
    echo "Starting existing Lima instance $profile"
    limactl start --tty=false --timeout=15m "$profile"
else
    echo "Creating disposable Lima instance $profile"
    limactl start --tty=false --timeout=20m \
        --name "$profile" \
        --vm-type qemu \
        --cpus "$cpus" \
        --memory "$memory" \
        --disk "$disk" \
        --mount-none \
        "$root/deploy/local-kubernetes/lima.yaml"
fi

if ! limactl shell "$profile" test -f /etc/locoder/development-vm; then
    echo "Lima instance $profile was not created by the Locoder development template." >&2
    echo "Choose another LIMA_INSTANCE or delete the disposable instance with make kube-dev-down." >&2
    exit 1
fi

instance_dir=$(limactl list "$profile" --format '{{.Dir}}')
kubeconfig="$instance_dir/copied-from-guest/kubeconfig.yaml"
test -f "$kubeconfig" || { echo "Lima did not export the Kubernetes kubeconfig: $kubeconfig" >&2; exit 1; }
kubectl_cmd=(kubectl --kubeconfig "$kubeconfig")
helm_cmd=(helm --kubeconfig "$kubeconfig")

node_name=$("${kubectl_cmd[@]}" get nodes -o jsonpath='{.items[0].metadata.name}')
test -n "$node_name" || { echo "Lima Kubernetes cluster has no node" >&2; exit 1; }
runtime=$("${kubectl_cmd[@]}" get node "$node_name" -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
[[ "$runtime" == containerd://* ]] || { echo "Lima node $node_name is not using containerd: $runtime" >&2; exit 1; }

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Installing and verifying gVisor inside the Lima VM"
node_arch=$(limactl shell "$profile" uname -m | tr -d '\r')
case "$node_arch" in
    x86_64|aarch64) ;;
    *) echo "gVisor does not publish a local development archive for $node_arch" >&2; exit 1 ;;
esac
gvisor_url="https://storage.googleapis.com/gvisor/releases/release/latest/$node_arch"
curl -fsSL "$gvisor_url/gvisor.tar.bz2" -o "$tmp/gvisor.tar.bz2"
curl -fsSL "$gvisor_url/gvisor.tar.bz2.sha512" -o "$tmp/gvisor.tar.bz2.sha512"
(
    cd "$tmp"
    sha512sum --check gvisor.tar.bz2.sha512
)
mkdir "$tmp/gvisor"
tar -xjf "$tmp/gvisor.tar.bz2" -C "$tmp/gvisor"
tar -cf "$tmp/gvisor.tar" -C "$tmp/gvisor" .
limactl copy "$tmp/gvisor.tar" "$profile:/tmp/locoder-gvisor.tar"
limactl copy "$root/scripts/lima-install-gvisor.sh" "$profile:/tmp/lima-install-gvisor.sh"
limactl shell "$profile" sudo bash /tmp/lima-install-gvisor.sh /tmp/locoder-gvisor.tar
"${kubectl_cmd[@]}" wait --for=condition=Ready node/"$node_name" --timeout=120s
"${kubectl_cmd[@]}" apply -f "$root/deploy/local-kubernetes/runtimeclass.yaml" >/dev/null
test "$("${kubectl_cmd[@]}" get runtimeclass gvisor -o jsonpath='{.handler}')" = gvisor || {
    echo "gvisor RuntimeClass is missing or has the wrong handler" >&2
    exit 1
}

"${kubectl_cmd[@]}" delete pod locoder-gvisor-check --ignore-not-found --wait=true --timeout=30s >/dev/null
"${kubectl_cmd[@]}" run locoder-gvisor-check \
    --image=docker.io/library/busybox:latest \
    --restart=Never \
    --overrides='{"spec":{"runtimeClassName":"gvisor","automountServiceAccountToken":false,"containers":[{"name":"locoder-gvisor-check","image":"docker.io/library/busybox:latest","command":["dmesg"],"securityContext":{"allowPrivilegeEscalation":false,"capabilities":{"drop":["ALL"]},"readOnlyRootFilesystem":true,"runAsNonRoot":true,"runAsUser":65534,"runAsGroup":65534,"seccompProfile":{"type":"RuntimeDefault"}}}]}}' >/dev/null
if ! "${kubectl_cmd[@]}" wait --for=jsonpath='{.status.phase}'=Succeeded pod/locoder-gvisor-check --timeout=60s; then
    "${kubectl_cmd[@]}" describe pod locoder-gvisor-check >&2
    exit 1
fi
"${kubectl_cmd[@]}" logs locoder-gvisor-check | grep -qi gvisor || {
    echo "RuntimeClass probe did not identify itself as gVisor" >&2
    exit 1
}
"${kubectl_cmd[@]}" delete pod locoder-gvisor-check --wait=false >/dev/null

archive_image() {
    local image=$1 archive=$2
    podman save --format oci-archive --output "$archive" "$image"
}
containerd_image_digest() {
    local image=$1
    limactl shell "$profile" sudo ctr --namespace k8s.io images list |
        awk -v ref="$image" '$1 == ref { print $3 }'
}

echo "Loading Locoder images into Lima's Kubernetes containerd"
archive_image "$app_image" "$tmp/app.tar"
archive_image "$toolchain_image" "$tmp/toolchain.tar"
limactl copy "$tmp/app.tar" "$tmp/toolchain.tar" "$profile:/tmp/"
limactl shell "$profile" sudo ctr --namespace k8s.io images import --digests /tmp/app.tar
limactl shell "$profile" sudo ctr --namespace k8s.io images import --digests /tmp/toolchain.tar
app_digest=$(containerd_image_digest "$app_image")
toolchain_digest=$(containerd_image_digest "$toolchain_image")
test -n "$app_digest" || { echo "containerd did not import $app_image" >&2; exit 1; }
test -n "$toolchain_digest" || { echo "containerd did not import $toolchain_image" >&2; exit 1; }
limactl shell "$profile" sudo ctr --namespace k8s.io images tag --force \
    "$app_image" "${app_image%:*}@$app_digest"
limactl shell "$profile" sudo ctr --namespace k8s.io images tag --force \
    "$toolchain_image" "${toolchain_image%:*}@$toolchain_digest"
limactl shell "$profile" sudo rm -f /tmp/app.tar /tmp/toolchain.tar /tmp/lima-install-gvisor.sh

echo "Creating ephemeral PostgreSQL and Valkey"
"${kubectl_cmd[@]}" create namespace locoder --dry-run=client -o yaml | "${kubectl_cmd[@]}" apply -f - >/dev/null
python3 "$root/scripts/kube-dev-secrets.py" "$root/.env" | "${kubectl_cmd[@]}" apply -f - >/dev/null
"${kubectl_cmd[@]}" apply -f "$root/deploy/local-kubernetes/dependencies.yaml" >/dev/null
"${kubectl_cmd[@]}" rollout status --namespace locoder deployment/locoder-postgres --timeout=180s
"${kubectl_cmd[@]}" rollout status --namespace locoder deployment/locoder-valkey --timeout=180s

echo "Installing Locoder"
"${helm_cmd[@]}" upgrade --install locoder "$root/deploy/helm/locoder" \
    --namespace locoder \
    --values "$root/deploy/local-kubernetes/values.yaml" \
    --set-string "images.app.repository=${app_image%:*}" \
    --set-string "images.app.digest=$app_digest" \
    --set-string "images.toolchain.repository=${toolchain_image%:*}" \
    --set-string "images.toolchain.digest=$toolchain_digest" \
    --wait \
    --timeout 5m

"${kubectl_cmd[@]}" get pods --namespace locoder
echo "Locoder is ready. Run: KUBECONFIG='$kubeconfig' kubectl -n locoder port-forward service/locoder-api 8080:8080"
