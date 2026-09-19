#!/usr/bin/env bash
# Build the pinned gVisor release with the rootless cgroup fixes.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
release=release-20260914.0
dev="$root/.dev"
source_dir="$dev/src/gvisor-$release"
archive="$source_dir/gvisor.tar.bz2"
builder_image=us-central1-docker.pkg.dev/gvisor-presubmit/gvisor-presubmit-images/default_x86_64@sha256:9afba722516843795c026fbe3b7cd8a8f6b729d241e656a70af1ad46fe09406c
patch_hash=$(sha256sum "$root/deploy/gvisor/rootless-systemd.patch" "$root/deploy/gvisor/rootless-cpuset.patch" | sha256sum | cut -d' ' -f1)
install_dir="$dev/gvisor/$release-rootless-${patch_hash:0:12}"

[[ $(uname -s) == Linux ]] || { echo 'gVisor development requires Linux' >&2; exit 1; }
[[ $(uname -m) == x86_64 ]] || { echo 'The pinned development build supports x86_64 only' >&2; exit 1; }
[[ $(id -u) != 0 ]] || { echo 'Run this development installer as your normal user' >&2; exit 1; }
for command in git podman sha256sum tar; do
 command -v "$command" >/dev/null || { echo "Missing required command: $command" >&2; exit 1; }
done
if [[ -x "$install_dir/runsc" && -d "$install_dir/gvisor-bin" && -f "$install_dir/.practice-patch-hash" ]] &&
 [[ $(<"$install_dir/.practice-patch-hash") == "$patch_hash" ]]; then
 printf 'gVisor development runtime already installed: %s\n' "$install_dir/runsc"
 exit 0
fi

mkdir -p "$dev/src" "$dev/cache" "$dev/gvisor"
if [[ ! -d "$source_dir/.git" ]]; then
 git clone --depth 1 --branch "$release" https://github.com/google/gvisor.git "$source_dir"
fi
cd "$source_dir"
expected=$(git rev-parse "$release^{}")
actual=$(git rev-parse HEAD)
[[ "$actual" == "$expected" ]] || { echo "Unexpected gVisor source revision: $actual" >&2; exit 1; }
apply_once() {
 local patch=$1
 if git apply --check "$patch" 2>/dev/null; then
  git apply "$patch"
 elif git apply --reverse --check "$patch" 2>/dev/null; then
  printf 'Patch already applied: %s\n' "${patch##*/}"
 else
  echo "Source contains changes that conflict with $patch" >&2
  exit 1
 fi
}
apply_once "$root/deploy/gvisor/rootless-systemd.patch"
apply_once "$root/deploy/gvisor/rootless-cpuset.patch"

mkdir -p "$dev/cache/gvisor-bazel"
podman pull "$builder_image"
podman run --rm --runtime=crun --user=0:0 --entrypoint=/bin/bash \
 -v "$source_dir:/workspace:Z" -w /workspace \
 -v "$dev/cache/gvisor-bazel:/root/.cache/bazel:Z" \
 "$builder_image" -lc \
 'bazel build -c opt //debian:gvisor-release-tar-bz2 && cp -L bazel-bin/debian/gvisor.tar.bz2 /workspace/gvisor.tar.bz2'

[[ -f "$archive" ]] || { echo "Build did not produce $archive" >&2; exit 1; }
staging="$install_dir.tmp"
rm -rf "$staging"
mkdir -p "$staging"
tar -xjf "$archive" -C "$staging"
[[ -x "$staging/runsc" && -d "$staging/gvisor-bin" ]] || {
 echo 'Built release is missing runsc or gvisor-bin' >&2
 exit 1
}
printf '%s\n' "$patch_hash" >"$staging/.practice-patch-hash"
mv "$staging" "$install_dir"
ln -sfn "$(basename "$install_dir")" "$dev/gvisor/current"
"$install_dir/runsc" --version
printf 'Installed project-local rootless gVisor: %s\n' "$install_dir/runsc"
