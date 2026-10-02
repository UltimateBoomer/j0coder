# Accounts and production security

Local accounts remain the default; the service has no dependency on an identity provider or SMTP server. New passwords require at least 15 characters and at most 256 UTF-8 bytes. Existing shorter passwords can still sign in. Passwords use Argon2id. Password hashing and verification have two execution slots and a bounded waiting queue.

## Registration and recovery

`REGISTRATION_MODE=invite` is the default. Only a valid, unexpired, single-use invitation creates a learner account. Set `closed` to refuse registration entirely. Public unrestricted registration is intentionally unavailable. Invitations expire after seven days; reset links after one hour. Tokens are random 256-bit values stored as SHA-256 hashes and consumed transactionally. A username conflict preserves the invitation. The browser receives tokens in URL fragments and removes the fragment before making requests.

Run account management on the server, as the installation owner:

```sh
podman compose exec api api manage help
podman compose exec api api manage invite
podman compose exec api api manage reset USERNAME
podman compose exec api api manage users
podman compose exec api api manage suspend USERNAME
podman compose exec api api manage unsuspend USERNAME
podman compose exec api api manage revoke USERNAME
podman compose exec api api manage role USERNAME learner
podman compose exec api api manage policy USERNAME submissions_day 200
podman compose exec api api manage policy USERNAME submissions_day null
podman compose exec api api manage policy-show USERNAME
podman compose exec api api manage audit
```

Invite/reset output is a bearer secret: deliver it privately and exclude it from logs. Creating an account manually reads the password from stdin; use a hidden terminal prompt and `podman compose exec -T` rather than a password argument. Native development uses `scripts/dev-service.sh api manage ...`. No account, role, quota, or catalog configuration writes are exposed over HTTP.

Suspension revokes sessions and cancels queued submissions. Work already admitted to a worker can finish; workers also check suspension before execution. Unsuspension requires a new sign-in. Password reset/change and “sign out all devices” revoke every session. The audit table records action names and affected user IDs, without tokens, passwords, or source code.

## Session and user boundaries

Public sessions last seven days with a 24-hour idle timeout; private sessions last one hour with a 15-minute idle timeout. At most five sessions per account survive, across both audiences. Cookies are HttpOnly, SameSite=Strict, and Secure for HTTPS origins. Bearer tokens are hashed in PostgreSQL. CSRF tokens and exact Origin checks protect authenticated writes. Authentication is checked before protected request-body parsing. API responses use `Cache-Control: no-store`.

Migration 0007 preserves accounts, problems, drafts, submissions, and versions, but deliberately removes old sessions. Back up before migrating; browsers must sign in again afterward. Never roll back the database by simply running an older binary.

Public sessions always report `admin=false`, even for an operator account. Public admin API paths and public metrics return 404. Authoring is managed with `api manage validate FILE`, `import FILE`, `save ID FILE`, and `publish ID`. Catalog configuration uses `api manage catalog FILE`; use generation 0 initially, then the latest stored generation for updates. Git-managed problems retain their existing authoring safeguards.

The optional `WEB_ADMIN_ENABLED=true` listener is separate from the public router, uses a separate `practice_admin` cookie/audience, and only accepts administrator accounts. It is disabled by default. Compose publishes it on host loopback port 8082; access through SSH:

```sh
ssh -L 8082:127.0.0.1:8082 operator@HOST
# Visit http://localhost:8082; ADMIN_ORIGIN must match this exact origin.
```

Private authoring writes require password authentication within the last five minutes. Reauthenticate from Settings. Account/role/policy changes remain host-only. Do not expose this listener with a public reverse proxy. Kubernetes provides a ClusterIP service and a distinct port-specific NetworkPolicy when enabled; ingress access only permits public ports 8080/8081. Use port-forwarding or explicitly labelled operator namespace/pods. The policy needs a CNI that enforces NetworkPolicy.

Ownership predicates protect cloud drafts, history, and submission detail regardless of administrator role. Published problem data excludes hidden cases; unpublished drafts and catalog diagnostics remain private. Execution controllers retain the existing sandbox boundary: no user code receives network access, secrets, controller sockets, or other users' payloads.

## Admission limits

Limits apply at the application layer, including direct API access, and use shared Valkey buckets across API replicas. Submission admissions additionally serialize in PostgreSQL, preserving idempotency without charging execution quotas twice. Database overrides are nullable and constrained; `null` inherits deployment defaults. `DEFAULT_USER_POLICY` can override a subset of defaults using a JSON object, for example `{"submissions_day":50}`. Unknown fields and out-of-range defaults refuse startup.

| Dimension | Default |
| --- | --- |
| Authenticated HTTP | 180/minute, burst 30 |
| Cloud draft writes | 120/minute, burst 20; also consume HTTP budget |
| Submissions | 6 per rolling minute, 100 per UTC day |
| Pending submissions | 2/user, 20 globally (`GLOBAL_PENDING_LIMIT`) |
| Semantic tickets | 6/minute, burst 2; also consume HTTP budget |
| Semantic sessions | 1/user, 3 globally (`EDITOR_CAPACITY`) |
| Editor messages | 20/second, burst 40, shared per user |
| Editor bytes | 1 MiB/second, burst 2 MiB, shared per user |
| Anonymous HTTP | 60/minute, burst 10/IP |
| Login | 5/minute, burst 5/IP; 10 failures/username in 15 minutes |
| Invitation/reset redemption | 3/hour/IP, plus 3/minute burst 3 |

All new executions, including compilation failures, consume admission quotas. Retries using the same idempotency key return the existing submission. Capacity failures return 503; rate failures return 429 with a reason and `Retry-After`. Browser cloud saves, polls, and semantic reconnects honor the retry delay. Revocation/logout remains available when HTTP quota is exhausted or Valkey is unavailable. Other admission paths fail closed when Valkey is unavailable.

The editor receives a short-lived authorization lease, bound to the originating session and account generation. Revocation, suspension, or disabling semantic completion stops lease renewal; an existing bridge closes after lease expiry and its next check (up to approximately 55 seconds). Browser disable closes its connection immediately, cancels pending requests/retries, clears diagnostics, and keeps basic editing. New tickets are refused while disabled. Lower session limits close surplus connections on renewal.

## Guest access and settings

`GUEST_BROWSING_ENABLED=false` by default. When enabled, unauthenticated users may list/read published problems and use the basic editor. Code stays in a separate browser-local guest namespace. There is no guest execution, semantic ticket, cloud save, history, or account settings API access. Signing in does not upload guest code into an account. Switching account/guest identity recreates the editor and its sync state.

Signed-in settings are stored per account: theme, default language, semantic completion, font size (10–24), tab width (2/4/8), wrap, minimap, and blind mode. Guests keep their preferences locally. Old browser theme preferences are imported only through the explicit Settings action.

## Proxy, bots, and DDoS

Compose uses a dedicated `j0coder_ingress_v2` network (10.89.44.0/24) and publishes public Nginx on loopback only. Its forwarding configuration expects a host TLS proxy such as Caddy, which must sanitize incoming `X-Forwarded-For`. Nginx preserves that chain rather than appending a translated rootless socket address. The API trusts only `TRUSTED_PROXY_CIDRS` and walks the chain from the nearest trusted hop; untrusted direct requests cannot select a spoofed IP. Adjust the subnet and trust CIDRs together if your host already uses that subnet. For a native API or Kubernetes, explicitly configure the actual proxy hops, and never use an unrestricted trust range.

The proxy has coarse request/connection limits, body/header timeouts and bounded body size. These protect origin resources but cannot absorb a volumetric attack. Put a CDN/WAF in front when publicly advertised, constrain origin ingress to that provider's current address ranges, and configure Caddy's trusted proxies accordingly. Bypass caching for all API and editor paths; never challenge a WebSocket upgrade or replace JSON API failures with challenge HTML. Invitation-only admission, IP/username throttles, bounded hashing, per-user quotas and sandbox isolation remain necessary even behind a CDN.

Anubis, Turnstile and OAuth options are evaluated in [hosting and identity research](hosting.md). No external CAPTCHA or identity service is required or enabled by this implementation.
