FROM docker.io/library/node@sha256:6c74791e557ce11fc957704f6d4fe134a7bc8d6f5ca4403205b2966bd488f6b3 AS web
WORKDIR /build/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run check && npm run build
FROM docker.io/library/rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS rust
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY migrations migrations
COPY openapi.json ./
RUN cargo build --locked --release
FROM docker.io/library/debian@sha256:b1a741487078b369e78119849663d7f1a5341ef2768798f7b7406c4240f86aef
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates podman && rm -rf /var/lib/apt/lists/*
COPY --from=rust /build/target/release/api /build/target/release/worker /build/target/release/editor /usr/local/bin/
COPY --from=web /build/web/dist /app/web
ENV WEB_DIR=/app/web
WORKDIR /app
USER 65534:65534
CMD ["api"]
