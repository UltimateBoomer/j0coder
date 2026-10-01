#!/usr/bin/env bash
set -euo pipefail

archive=${1:-/tmp/j0coder-gvisor.tar}
test -f "$archive" || { echo "missing gVisor archive: $archive" >&2; exit 1; }

install -d -m 0755 /usr/local/bin /etc/containerd/conf.d
tar -xf "$archive" -C /usr/local/bin
test -x /usr/local/bin/runsc
test -x /usr/local/bin/containerd-shim-runsc-v1
test -d /usr/local/bin/gvisor-bin

cat >/etc/containerd/conf.d/99-j0coder-gvisor.toml <<'EOF'
version = 2

[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.gvisor]
runtime_type = "io.containerd.runsc.v1"
EOF

systemctl restart containerd
for _ in $(seq 1 30); do
    if ctr plugins list | grep -q 'io.containerd.grpc.v1.*cri.*ok'; then
        rm -f "$archive"
        exit 0
    fi
    sleep 1
done

systemctl --no-pager --full status containerd >&2 || true
journalctl --no-pager -u containerd -n 100 >&2 || true
echo "containerd did not become ready after installing gVisor" >&2
exit 1
