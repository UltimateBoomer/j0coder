# Ephemeral Kubernetes deployment test on Fedora

Use this disposable cluster to test the staging and production Helm deployment locally. For ordinary development, start with [Podman Compose](../README.md#develop-locally). The Lima-managed QEMU VM contains a single-node Kubernetes 1.34 kubeadm cluster, system containerd, Flannel, and gVisor. The minor version is pinned in the repository overlay so Lima template updates cannot silently change it. QEMU runs as the developer and uses `/dev/kvm` for hardware acceleration. The workflow does not connect to system libvirt or require membership in the `libvirt` group. Podman is used only to build and export the local application and toolchain images.

## One-time host setup

Install QEMU/KVM and the development tools:

```bash
sudo dnf install -y qemu-system-x86-core podman kubectl helm curl bzip2
```

On ARM64, install Fedora's corresponding AArch64 QEMU system package. Install Lima 2.0 or newer using the [official installation instructions](https://lima-vm.io/docs/installation/). Lima's additional guest agents are required when using an architecture different from the host; native x86-64 and ARM64 installations use the normal guest agent included in the main archive.

Load the CPU-specific KVM module. AMD hosts use `kvm_amd`; Intel hosts use `kvm_intel`:

```bash
sudo modprobe kvm_amd       # or: sudo modprobe kvm_intel
```

Add the developer account to the group that permits KVM acceleration, then log out and back in:

```bash
sudo usermod -aG kvm "$USER"
```

Do not add the developer to the `libvirt` group for this workflow. Fedora normally grants that group passwordless write access to system libvirt, which is broader and potentially root-equivalent authority that Lima's direct QEMU backend does not need.

Validate the session:

```bash
limactl --version
limactl validate deploy/local-kubernetes/lima.yaml
grep -E -q 'vmx|svm' /proc/cpuinfo
test -r /dev/kvm && test -w /dev/kvm
qemu-system-x86_64 --version
```

AMD-V or Intel VT-x must be enabled in firmware. Routine startup and teardown run without host `sudo`.

## Normal workflow

Create `.env` if needed and start the environment:

```bash
make configure
make kube-dev-up
```

`kube-dev-up`:

1. validates Lima, the repository template, QEMU, virtualization flags, and `/dev/kvm` access;
2. builds the shared application image and toolchain image with Podman;
3. creates a Lima QEMU VM from the repository's kubeadm template overlay, with no host mounts;
4. verifies the VM contains a ready Kubernetes node using system containerd;
5. installs the checksummed gVisor archive, configures its containerd runtime handler, creates the RuntimeClass, and runs an identity probe;
6. copies OCI archives into the VM and imports them into containerd's `k8s.io` namespace;
7. reads PostgreSQL and Valkey credentials from `.env`, starts both services with `emptyDir` storage, and installs the production Helm chart with local values.

For machine-specific Helm settings, create `deploy/local-kubernetes/values.local.yaml`. It is gitignored and applied after the tracked `values.yaml` on every `make kube-dev-up`. For example, to enable a private problem catalog:

```yaml
existingSecrets:
  catalogSsh: {name: locoder-catalog-ssh, privateKeyKey: private_key, knownHostsKey: known_hosts}
catalog:
  enabled: true
  repositoryUrl: git@github.com:YOUR_ACCOUNT/YOUR_REPO.git
  revision: main
  strategy: track_branch
```

Create the referenced Kubernetes SSH Secret separately; do not put private keys in either values file. The override is local to this checkout and must be recreated on a new machine.

Lima forwards the Kubernetes API to localhost and exports a kubeconfig under its instance directory. The startup script prints its exact path. To locate it again:

```bash
export KUBECONFIG="$(limactl list locoder --format '{{.Dir}}')/copied-from-guest/kubeconfig.yaml"
kubectl get nodes
```

Expose Locoder through the Kubernetes API connection:

```bash
kubectl -n locoder port-forward service/locoder-api 8080:8080
```

Open `http://localhost:8080`. This API-only port-forward does not provide semantic completion. To use it, run these in separate terminals instead:

```bash
kubectl -n locoder port-forward service/locoder-api 18080:8080
kubectl -n locoder port-forward service/locoder-editor 8081:8081
npm run dev --prefix web
```

Open `http://localhost:8080` through Vite, which sends `/api` to the API forward and `/editor` to the editor forward. Both routes must share the browser origin; two independent port-forwards without this routing will not connect the editor WebSocket.

For a new cluster, create its first administrator in another terminal using the same `KUBECONFIG`:

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

Sign in as `admin`. The cluster and its database are deleted by `make kube-dev-down`, so a recreated cluster needs a new administrator.

Resource settings and the instance name can be overridden on initial creation:

```bash
LIMA_INSTANCE=locoder \
LIMA_CPUS=8 \
LIMA_MEMORY=16 \
LIMA_DISK_SIZE=60 \
make kube-dev-up
```

Resource overrides do not resize an existing instance. Delete and recreate the disposable VM to apply them. Startup also refuses to reuse a VM with another backend or without the Locoder template marker:

```bash
make kube-dev-down
make kube-dev-up
```

Delete the VM, Kubernetes cluster, and all ephemeral database data when finished:

```bash
make kube-dev-down
```

## Security boundary

The QEMU and Lima host-agent processes run with the developer's UID. A VM escape would therefore reach the developer account rather than directly become host root. That remains a serious compromise because the account can access source, credentials, and other user-owned files. Keep Lima, QEMU, and the host kernel updated.

The repository template explicitly disables host mounts. Do not add home-directory mounts, repository mounts, SSH-agent forwarding, or host runtime sockets. Access to `/dev/kvm` exposes the kernel KVM interface but does not grant system-libvirt management authority. Guest `sudo`, kubeadm, containerd, and the gVisor installation operate inside the disposable VM.

Locoder's chart containers and dynamically created sandbox Pods remain non-root and unprivileged. They disallow privilege escalation, drop all capabilities, use read-only root filesystems, and receive only explicit writable temporary volumes. The chart does not mount a container-runtime socket or add privileged containers. The sandbox namespace retains its default-deny NetworkPolicy.

## Troubleshooting

- **`/dev/kvm` missing:** enable virtualization in firmware, load `kvm` plus `kvm_amd` or `kvm_intel`, and check `dmesg` for KVM errors.
- **`/dev/kvm` permission denied:** confirm `id` shows the `kvm` group after a full logout/login and inspect `ls -l /dev/kvm`.
- **Lima provisioning fails:** run `limactl shell locoder sudo tail -n 200 /var/log/cloud-init-output.log` and inspect `~/.lima/locoder/ha.stderr.log`.
- **Kubernetes API unavailable:** verify `limactl list locoder`, confirm the copied kubeconfig exists, and retry `kubectl --kubeconfig PATH get nodes`.
- **Containerd fails after gVisor installation:** inspect `limactl shell locoder sudo journalctl -u containerd -n 200` and `/etc/containerd/conf.d/99-locoder-gvisor.toml`.
- **Image is reported missing:** check `limactl shell locoder sudo ctr --namespace k8s.io images list` and recreate the VM if an interrupted import left inconsistent state.
- **VPN or DNS failures:** Lima user-mode networking uses the host resolver. Test without the VPN, then correct host resolver or VPN split-DNS policy.
- **Existing instance rejected:** choose a different `LIMA_INSTANCE` or delete the disposable instance with `make kube-dev-down`.

## Why Lima

| Option | Host privilege and lifecycle | Fit |
|---|---|---|
| Lima QEMU with kubeadm | Direct user-owned QEMU, automatic API forwarding, file copy, and VM lifecycle | Selected; explicit containerd/gVisor integration without system libvirt |
| Minikube QEMU2 | Direct user-owned QEMU with Minikube lifecycle | Functional driver path, but less mature on Linux and its builtin network has limited service support |
| Minikube KVM2 | Uses privileged `qemu:///system` libvirt management | Rejected because passwordless `libvirt` group access is too broad |
| KVM2 with `qemu:///session` | Rootless libvirt session | Minikube does not reliably support the required networking |
| Rootless container-based clusters | Nested inside delegated host cgroups | Rejected because they do not resolve the observed gVisor `cpuset` failure |
