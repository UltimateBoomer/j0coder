ARG APP_IMAGE=localhost/locoder-app:1
FROM ${APP_IMAGE} AS podman-app
FROM docker.io/library/debian@sha256:b1a741487078b369e78119849663d7f1a5341ef2768798f7b7406c4240f86aef
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git openssh-client && rm -rf /var/lib/apt/lists/*
COPY --from=podman-app /usr/local/bin/api /usr/local/bin/worker /usr/local/bin/editor /usr/local/bin/catalog-controller /usr/local/bin/
COPY --from=podman-app /app/web /app/web
ENV WEB_DIR=/app/web
WORKDIR /app
USER 65534:65534
CMD ["api"]
