FROM docker.io/library/debian@sha256:b1a741487078b369e78119849663d7f1a5341ef2768798f7b7406c4240f86aef
RUN apt-get update && apt-get install -y --no-install-recommends clang-16 clangd-16 python3 nodejs npm nlohmann-json3-dev ca-certificates && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,id=locoder-toolchain-npm,target=/root/.npm npm install -g pyright@1.1.407
RUN ln -s /usr/bin/clang++-16 /usr/local/bin/clang++ \
    && ln -s /usr/bin/clangd-16 /usr/local/bin/clangd \
    && mkdir /input /workspace \
    && touch /workspace/solution.cpp /workspace/solution.py \
    && chmod 1777 /workspace
COPY deploy/harness.py /opt/harness.py
COPY deploy/compile_flags.txt /workspace/compile_flags.txt
COPY deploy/pyrightconfig.json /workspace/pyrightconfig.json
ENV PYTHONDONTWRITEBYTECODE=1
USER 65534:65534
