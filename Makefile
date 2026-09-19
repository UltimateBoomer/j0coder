.PHONY: check build test images up dev-gvisor dev-up dev-down
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
images:
	podman build -t localhost/practice-toolchain:1 -f deploy/Toolchain.Containerfile .
	podman build -t localhost/practice-app:1 -f deploy/App.Containerfile .
	python3 scripts/pin-images.py
up:
	./scripts/preflight.sh
	podman compose up -d
dev-gvisor:
	./scripts/install-gvisor-dev.sh
dev-up:
	./scripts/dev-up.sh
dev-down:
	./scripts/dev-down.sh
