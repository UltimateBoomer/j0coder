PODMAN ?= podman
BUILD_JOBS ?= 2
APP_IMAGE ?= localhost/locoder-app:1
KUBERNETES_APP_IMAGE ?= localhost/locoder-app-kubernetes:1
TOOLCHAIN_IMAGE ?= localhost/locoder-toolchain:1
CACHE_FROM_REPO ?=
CACHE_TO_REPO ?=

APP_CACHE_FLAGS = $(if $(CACHE_FROM_REPO),--cache-from=$(CACHE_FROM_REPO)/app) $(if $(CACHE_TO_REPO),--cache-to=$(CACHE_TO_REPO)/app)
TOOLCHAIN_CACHE_FLAGS = $(if $(CACHE_FROM_REPO),--cache-from=$(CACHE_FROM_REPO)/toolchain) $(if $(CACHE_TO_REPO),--cache-to=$(CACHE_TO_REPO)/toolchain)

.DEFAULT_GOAL := help

.PHONY: help configure check build test app-image kubernetes-app-image toolchain-image images pin-images helm-check
.PHONY: preflight up dev-gvisor dev-up dev-down

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
	  '  kubernetes-app-image Build the Podman-free application image' \
	  '  toolchain-image Build the sandbox toolchain image' \
	  '  helm-check      Lint and render the Kubernetes chart' \
	  '  images          Configure, build both images, and pin their IDs' \
	  '' \
	  'Runtime:' \
	  '  preflight       Validate the configured gVisor sandbox' \
	  '  up              Run preflight, then start the configured stack' \
	  '  dev-up          Build images, install gVisor, and start the dev stack' \
	  '  dev-down        Stop the dev stack and its gVisor controller'

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
	$(PODMAN) build --layers --jobs=$(BUILD_JOBS) $(APP_CACHE_FLAGS) -t $(APP_IMAGE) -f deploy/App.Containerfile .

kubernetes-app-image: app-image
	$(PODMAN) build --layers --jobs=$(BUILD_JOBS) --build-arg APP_IMAGE=$(APP_IMAGE) -t $(KUBERNETES_APP_IMAGE) -f deploy/KubernetesApp.Containerfile .

helm-check:
	helm lint deploy/helm/locoder
	helm template locoder deploy/helm/locoder >/dev/null

toolchain-image:
	$(PODMAN) build --layers --jobs=$(BUILD_JOBS) $(TOOLCHAIN_CACHE_FLAGS) -t $(TOOLCHAIN_IMAGE) -f deploy/Toolchain.Containerfile .

images: pin-images

pin-images: configure toolchain-image app-image
	python3 scripts/pin-images.py --podman '$(PODMAN)' --app-image '$(APP_IMAGE)' --toolchain-image '$(TOOLCHAIN_IMAGE)'

# Deployment runtime
preflight:
	./scripts/preflight.sh

up: preflight
	$(PODMAN) compose up -d

# Rootless development runtime
dev-gvisor:
	./scripts/install-gvisor-dev.sh

# Runtime startup and health checks remain together so failures can stop the
# controller and Compose services through one shell trap.
dev-up: images dev-gvisor
	./scripts/dev-up.sh

dev-down:
	./scripts/dev-down.sh
