PODMAN ?= podman
BUILD_JOBS ?= 2
LIMA_INSTANCE ?= j0coder
LIMA_CPUS ?= 6
LIMA_MEMORY ?= 12
LIMA_DISK_SIZE ?= 40
APP_IMAGE ?= localhost/j0coder-app:1
TOOLCHAIN_IMAGE ?= localhost/j0coder-toolchain:1

.DEFAULT_GOAL := help

.PHONY: help configure check build test app-image toolchain-image images pin-images helm-check
.PHONY: preflight up down status logs dev-gvisor dev-up dev-attach dev-down kube-dev-up kube-dev-down

# Repository setup
help:
	@printf '%s\n' \
	  'Setup:' \
	  '  configure       Create .env and local service configuration' \
	  '  dev-gvisor      Install the project-local patched gVisor runtime' \
	  '' \
	  'Quality:' \
	  '  check           Run Rust and frontend static checks' \
	  '  build           Build Rust and frontend development artifacts' \
	  '  test            Run Rust tests' \
	  '' \
	  'Images:' \
	  '  app-image       Build the application image' \
	  '  toolchain-image Build the sandbox toolchain image' \
	  '  helm-check      Lint and render the Kubernetes chart' \
	  '  images          Configure, build both images, and pin their IDs' \
	  '' \
	  'Runtime:' \
	  '  preflight       Validate the configured gVisor sandbox' \
	  '  up              Install and start the release-backed stack' \
	  '  dev-up          Start native services and Compose database/queue dependencies' \
	  '  dev-attach      Attach to the development tmux session' \
	  '  dev-down        Stop native services, dependencies, and gVisor controller' \
	  '  kube-dev-up     Build and start an ephemeral Lima Kubernetes stack' \
	  '  kube-dev-down   Delete the ephemeral Lima VM and its data'

configure: .env

.env:
	python3 scripts/configure.py

# Local quality gates
check:
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	npm run check --prefix web

build:
	cargo build --locked
	npm ci --prefix web
	npm run build --prefix web

test:
	cargo test --locked

# Container images
app-image:
	$(PODMAN) build --layers --jobs=$(BUILD_JOBS) -t $(APP_IMAGE) -f deploy/App.Containerfile .

helm-check:
	helm lint deploy/helm/j0coder
	helm template j0coder deploy/helm/j0coder >/dev/null

toolchain-image:
	$(PODMAN) build --layers --jobs=$(BUILD_JOBS) -t $(TOOLCHAIN_IMAGE) -f deploy/Toolchain.Containerfile .

images: pin-images

pin-images: configure toolchain-image app-image
	python3 scripts/pin-images.py --podman '$(PODMAN)' --app-image '$(APP_IMAGE)' --toolchain-image '$(TOOLCHAIN_IMAGE)'

# Deployment runtime
preflight:
	./scripts/preflight.sh

up:
	./scripts/setup.sh up

down status logs:
	./scripts/setup.sh $@

# Rootless development runtime
dev-gvisor:
	./scripts/install-gvisor-dev.sh

# Runtime startup and health checks remain together so failures can stop the
# native services, controller, and Compose dependencies through one shell trap.
dev-up: configure toolchain-image dev-gvisor
	TOOLCHAIN_IMAGE='$(TOOLCHAIN_IMAGE)' ./scripts/dev-up.sh

dev-attach:
	tmux -L j0coder-dev attach -t j0coder

dev-down:
	./scripts/dev-down.sh

# PostgreSQL and Valkey use emptyDir storage. Teardown deletes the complete
# Lima VM and all of its data.
kube-dev-up: configure helm-check toolchain-image app-image
	LIMA_INSTANCE='$(LIMA_INSTANCE)' \
	LIMA_CPUS='$(LIMA_CPUS)' \
	LIMA_MEMORY='$(LIMA_MEMORY)' \
	LIMA_DISK_SIZE='$(LIMA_DISK_SIZE)' \
	APP_IMAGE='$(APP_IMAGE)' \
	TOOLCHAIN_IMAGE='$(TOOLCHAIN_IMAGE)' \
	./scripts/kube-dev-up.sh

kube-dev-down:
	LIMA_INSTANCE='$(LIMA_INSTANCE)' ./scripts/kube-dev-down.sh
