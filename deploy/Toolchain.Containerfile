FROM docker.io/library/debian@sha256:b1a741487078b369e78119849663d7f1a5341ef2768798f7b7406c4240f86aef
RUN apt-get update && apt-get install -y --no-install-recommends clang-16 clangd-16 python3 nodejs npm nlohmann-json3-dev ca-certificates curl unzip tar && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,id=locoder-toolchain-npm,target=/root/.npm npm install -g pyright@1.1.407
# JDK 25 runs the compiler and language servers. Bytecode targets JVM 21.
RUN curl -fsSL 'https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.2%2B10/OpenJDK25U-jdk_x64_linux_hotspot_25.0.2_10.tar.gz' -o /tmp/jdk.tar.gz \
    && echo '987387933b64b9833846dee373b640440d3e1fd48a04804ec01a6dbf718e8ab8  /tmp/jdk.tar.gz' | sha256sum -c - \
    && mkdir -p /opt/jdk && tar -xzf /tmp/jdk.tar.gz -C /opt/jdk --strip-components=1 && rm /tmp/jdk.tar.gz
ENV JAVA_HOME=/opt/jdk
ENV PATH=/opt/jdk/bin:/opt/kotlinc/bin:$PATH
RUN curl -fsSL 'https://github.com/JetBrains/kotlin/releases/download/v2.4.20/kotlin-compiler-2.4.20.zip' -o /tmp/kotlinc.zip \
    && echo '59e9ca74c7904ef2c122b12114937673ccce68de820a663f0ed66ccf8799e0b7  /tmp/kotlinc.zip' | sha256sum -c - \
    && unzip -q /tmp/kotlinc.zip -d /opt && rm /tmp/kotlinc.zip
RUN mkdir -p /opt/jdtls && curl -fsSL 'https://download.eclipse.org/jdtls/milestones/1.56.0/jdt-language-server-1.56.0-202601291528.tar.gz' -o /tmp/jdtls.tar.gz \
    && echo '988492f1a4350be52aafbe538dc04f5c4dae08ad0e1f2aa35317622ed2dbe3c6  /tmp/jdtls.tar.gz' | sha256sum -c - \
    && tar --no-same-owner -xzf /tmp/jdtls.tar.gz -C /opt/jdtls && rm /tmp/jdtls.tar.gz
RUN mkdir -p /opt/kotlin-lsp && curl -fsSL 'https://download.jetbrains.com/language-server/kotlin-server/263.4702.0/kotlin-server-263.4702.0.tar.gz' -o /tmp/kotlin-lsp.tar.gz \
    && echo '1e11d2e5fefbf9ea215ad8dd6be95f2222897cd086e8cb7a661a52084a590405  /tmp/kotlin-lsp.tar.gz' | sha256sum -c - \
    && tar --no-same-owner -xzf /tmp/kotlin-lsp.tar.gz -C /opt/kotlin-lsp && rm /tmp/kotlin-lsp.tar.gz \
    && find /opt/kotlin-lsp -name kotlin-lsp.sh -exec chmod +x '{}' \; \
    && ln -s "$(find /opt/kotlin-lsp -name kotlin-lsp.sh -print -quit)" /usr/local/bin/locoder-kotlin-lsp
RUN mkdir -p /opt/jackson /opt/judge \
    && for module in jackson-core jackson-databind; do curl -fsSL "https://repo.maven.apache.org/maven2/com/fasterxml/jackson/core/$module/2.20.1/$module-2.20.1.jar" -o "/opt/jackson/$module.jar"; done \
    && curl -fsSL 'https://repo.maven.apache.org/maven2/com/fasterxml/jackson/core/jackson-annotations/2.20/jackson-annotations-2.20.jar' -o /opt/jackson/jackson-annotations.jar \
    && echo '959a2ffb2d591436f51f183c6a521fc89347912f711bf0cae008cdf045d95319  /opt/jackson/jackson-annotations.jar' | sha256sum -c - \
    && echo 'ffab4d957daa2796cf24cb66d0b78a7090f1bcbe17c3a4578f09affaaf137089  /opt/jackson/jackson-core.jar' | sha256sum -c - \
    && echo '34bbeb4526fff4f8565b12106bf85a6afcbae858966d489b54214ac46b2e26e8  /opt/jackson/jackson-databind.jar' | sha256sum -c -
COPY deploy/JudgeMain.java /opt/judge/JudgeMain.java
RUN javac --release 21 -cp '/opt/jackson/*' -d /opt/judge /opt/judge/JudgeMain.java
RUN ln -s /usr/bin/clang++-16 /usr/local/bin/clang++ \
    && ln -s /usr/bin/clangd-16 /usr/local/bin/clangd \
    && mkdir /input /workspace \
    && touch /workspace/solution.cpp /workspace/solution.py /workspace/Solution.java /workspace/solution.kt \
    && chmod 1777 /workspace
COPY deploy/harness.py /opt/harness.py
COPY deploy/compile_flags.txt /workspace/compile_flags.txt
COPY deploy/pyrightconfig.json /workspace/pyrightconfig.json
ENV PYTHONDONTWRITEBYTECODE=1
USER 65534:65534
