# Architecture and scaling

Locoder separates the trusted control plane from untrusted user execution. PostgreSQL is the durable authority, Valkey coordinates asynchronous work, and gVisor provides the execution boundary. Local Compose development controls gVisor through rootless Podman; staging and production control it through Kubernetes.

`SANDBOX_BACKEND=podman` is the local default. The Kubernetes deployment sets `SANDBOX_BACKEND=kubernetes`, uses in-cluster credentials, and creates a fresh Pod for compilation, each function case or stateful trace, and each editor session. Public API contracts, queued jobs, comparison logic, and verdicts are unchanged.

In Kubernetes, workers and editors hold namespace-scoped Pod and attach permissions in a dedicated Restricted-PSA sandbox namespace. API and catalog processes have no Kubernetes API credentials. Sandbox Pods have no service-account token or network access, require the configured gVisor RuntimeClass, and exchange submission material only over attach stdin/stdout. A controller timeout is recorded separately from an OOM termination; every path attempts immediate deletion, while expiry labels and a periodic sweeper recover after controller crashes.

## Local Compose topology

```mermaid
flowchart LR
    Browser[Browser<br/>Svelte + Monaco]
    Proxy[Nginx<br/>same-origin ingress]
    API[API replicas<br/>Axum]
    Editor[Editor replicas<br/>LSP sessions]
    Catalog[Catalog controller<br/>single reconciler]
    DB[(PostgreSQL<br/>durable authority)]
    Queue[(Valkey Streams<br/>jobs, events, tickets)]
    Worker[Worker replicas]
    Podman[Rootless Podman API<br/>runsc default runtime]
    Sandbox[Ephemeral gVisor sandboxes<br/>compiler, solution, LSP]
    Git[Problem catalog<br/>SSH Git repository]

    Browser -->|HTTPS API| Proxy
    Browser -->|WebSocket| Proxy
    Proxy --> API
    Proxy --> Editor
    API --> DB
    API --> Queue
    Editor --> Queue
    Catalog --> DB
    Catalog -->|fetch| Git
    Worker --> Queue
    Worker --> DB
    Worker --> Podman
    Editor --> Podman
    Podman --> Sandbox

    classDef untrusted fill:#fff2cc,stroke:#a67c00
    classDef durable fill:#d9ead3,stroke:#38761d
    classDef coordination fill:#cfe2f3,stroke:#0b5394
    class Sandbox untrusted
    class DB durable
    class Queue coordination
```

The API never receives the Podman socket. Workers and editors do receive it and are trusted controller components. Submitted programs run without networking, credentials, the Podman socket, or access to other tests. On Kubernetes, the workers and editors instead use their scoped service accounts to manage sandbox Pods.

## Submission lifecycle

```mermaid
sequenceDiagram
    actor User
    participant API
    participant DB as PostgreSQL
    participant V as Valkey Streams
    participant W as Worker
    participant P as gVisor sandbox backend

    User->>API: Submit source and immutable version
    API->>DB: Commit submission and outbox row
    API-->>User: 202 Accepted
    API->>V: Dispatch versioned job
    W->>V: Acquire job and renewable lease
    W->>DB: Load source and complete tests
    loop Each test case
        W->>P: Create fresh sandbox
        P-->>W: Output, logs, resource status
    end
    W->>V: Publish result and acknowledge atomically
    API->>V: Consume result event
    API->>DB: Persist final outcome
    User->>API: Poll submission
    API-->>User: Verdict and redacted case results
```

Submission creation and its outbox record commit in one database transaction. Dispatch is at least once. Worker leases, generation IDs, and attempt tokens fence stale executions; accepted final results are immutable. A database reconciliation pass republishes submissions that remain incomplete after the recovery window, allowing PostgreSQL to recover work even if Valkey loses stream state.

Function cases execute one call per fresh sandbox. Stateful cases construct one fresh instance and run the complete ordered operation trace in that sandbox. C++ compilation happens once per submission, and the trusted worker transfers the artifact into subsequent case sandboxes.

## Editor lifecycle

```mermaid
sequenceDiagram
    actor User
    participant API
    participant V as Valkey
    participant E as Editor service
    participant P as gVisor sandbox backend
    participant LSP as clangd or Pyright

    User->>API: Request editor ticket
    API->>V: Store one-use ticket
    API-->>User: Ticket and workspace path
    User->>E: Open authenticated WebSocket
    E->>V: Consume ticket
    E->>P: Start isolated LSP sandbox
    P->>LSP: Launch language server
    User->>E: Filtered LSP requests
    E-->>User: Filtered LSP responses
    E->>LSP: LSP requests
    LSP-->>E: LSP responses
    E->>P: Destroy sandbox on close or timeout
```

The editor has no PostgreSQL credentials. Tickets are short-lived and one use. The service restricts LSP methods and workspace URIs. Valkey enforces a shared session cap and a two-session per-user cap with renewable 45-second reservations. Sessions close after five minutes without client messages; the sandbox has a one-hour runtime limit. Editor replicas periodically remove sandboxes whose reservations have expired.

## Catalog reconciliation

The catalog controller fetches an SSH Git revision into a temporary bare repository, checks out the selected commit, validates the manifest and artifacts, and reconciles immutable problem versions in PostgreSQL.

```mermaid
flowchart TD
    Trigger[Poll interval or settings change] --> Lock{Acquire PostgreSQL<br/>advisory lock}
    Lock -->|No| Record[Record locked run]
    Lock -->|Yes| Fetch[Fetch configured Git revision]
    Fetch --> Validate[Validate paths, checksums,<br/>contracts, and undeclared files]
    Validate --> Diff{Compare artifact hashes}
    Diff -->|new or changed| Publish[Publish immutable versions]
    Diff -->|unchanged| Keep[Keep current versions]
    Publish --> Provenance[Store commit and manifest provenance]
    Keep --> Provenance
```

The intended deployment runs exactly one catalog controller. Its database advisory lock is a defensive guard against overlapping reconciliation attempts during restarts or accidental duplicate processes.

## Scaling model

### API

API replicas are stateless apart from PostgreSQL and Valkey connections and can scale horizontally behind a reverse proxy. Session state lives in PostgreSQL, so HTTP requests do not require sticky routing. Scale this for HTTP concurrency, catalog browsing, polling, and result persistence. Size PostgreSQL connection limits before adding replicas.

Each API process currently runs dispatch and result-consumer loops. Stream coordination and database idempotency prevent duplicate finalization, but replicas increase both connection count and background-consumer activity.

### Workers

Workers are the primary submission-throughput scaling unit. They share the Valkey consumer group and can be added independently. For local Compose development:

```sh
podman compose up -d --scale worker=4
```

`WORKER_CONCURRENCY` controls concurrent submissions inside each worker. Prefer more processes with conservative per-worker concurrency when failure containment matters. Approximate execution capacity with:

```text
worker replicas × WORKER_CONCURRENCY × active sandboxes per submission
```

A submission normally has one execution sandbox active at a time, plus compilation when required. Limits apply per sandbox, but execution nodes still need headroom for gVisor, workers, and compilation bursts.

The Podman socket in the Compose deployment is node-local and must never be exposed over a public network. The Kubernetes chart uses service accounts and sandbox Pods for multi-node scaling; see its [capacity and scheduling guidance](../deploy/helm/locoder/README.md#capacity-and-scheduling).

### Editors

Editor replicas can scale horizontally, but each WebSocket remains attached to one replica for its lifetime. Tickets are shared through Valkey, so the initial connection can be load-balanced without sticky routing; the proxy must preserve the upgraded connection afterward.

Valkey enforces one shared `EDITOR_CAPACITY` limit across replicas (eight by default). Compose editors use the node-local Podman socket; Kubernetes editors use scoped permissions in the sandbox namespace.

### PostgreSQL and Valkey

PostgreSQL is the source of truth and the first stateful bottleneck. Use durable storage, monitored backups, appropriate connection pooling, and a tested failover strategy before increasing application replicas. Read replicas do not directly accelerate the current write-heavy submission path without application changes.

Valkey carries jobs, result events, leases, and editor tickets. Enable persistence and disable eviction. It is recoverable coordination state rather than the durable submission authority, but losing it delays work until reconciliation. A highly available deployment reduces that delay.

### Catalog controller

Catalog reconciliation is a singleton responsibility and should not be horizontally scaled. PostgreSQL advisory locking defensively serializes overlapping attempts. Very large catalogs would benefit from incremental fetch and validation rather than additional controllers.

## Deployment topology

The Compose topology targets one rootless Linux host. It can scale workers and editors on that host, but it does not schedule across machines. Staging and production use the [Kubernetes chart](../deploy/helm/locoder/README.md), which schedules application replicas and gVisor sandbox Pods across nodes. That deployment needs:

- shared PostgreSQL and Valkey services;
- ingress for API and editor replicas;
- a working gVisor RuntimeClass on sandbox nodes;
- immutable application and toolchain image digests available to every scheduled node;
- centralized metrics and logs that continue to redact source and hidden tests.

## Security and resource boundaries

Sandboxes run as UID 65534 with a read-only root filesystem, dropped capabilities, no-new-privileges, no networking, bounded tmpfs, and CPU, memory, PID, file, output, and wall-clock limits. The local controller verifies that Podman recorded runsc as the OCI runtime; the Kubernetes controller requires the configured gVisor RuntimeClass. Compilation receives source but no tests; execution wrappers receive only the current case through standard input.

Hidden inputs, expected values, outputs, and runtime diagnostics are redacted from learner-facing responses. Service logs must not include source, test content, WebSocket tickets, or sandbox runtime output.
