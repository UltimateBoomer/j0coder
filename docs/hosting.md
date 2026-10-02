# Hosting, identity and abuse controls

Research checked 2026-10-02. The recommendation for fewer than ten accounts is one x86_64 Ubuntu host with PostgreSQL, Valkey, API, worker, editor and the existing gVisor sandbox runtime. Budget for concurrent compilers and JVM language servers, not only HTTP traffic. Start with 16 GB RAM, three global semantic sessions, two worker slots and strict user quotas; inspect memory pressure and latency before raising concurrency. This is a sizing hypothesis, not a hosted load-test result. No GPU or external model server is required by this service.

## Hosting options

| Option | Published compute cost | Interpretation |
| --- | --- | --- |
| Hetzner CX43, Germany/Finland | €15.99/month, excluding IPv4 and VAT | Selected Terraform default; shared x86 CPU, 16 GB RAM. |
| Hetzner CX33 | €8.49/month, excluding IPv4 and VAT | Lower-cost experiment; less headroom for simultaneous JVM editors/compilers. |
| AWS Lightsail, Linux general purpose, IPv4 | $84/month, 16 GB / 4 vCPU / 320 GB disk | Predictable AWS alternative; rootless runtime still requires host validation. |
| AWS Lightsail, 8 GB | $44/month, 2 vCPU / 160 GB disk | Reduced concurrency; measure rather than assuming parity with the 16 GB host. |
| AWS EKS | $0.10/hour in standard support, about $73/month at 730 hours | Control plane alone; workers, disks, IPv4 and other resources cost extra. |

Sources: [Hetzner June 2026 pricing](https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/), [Lightsail pricing](https://aws.amazon.com/lightsail/pricing/), [EKS pricing](https://aws.amazon.com/eks/pricing/). Prices vary by location, tax, architecture and future provider changes; these currencies are not converted.

For the selected CX43, a planning estimate is €15.99 compute + €3.20 server backups (20%) + roughly €0.50 IPv4 + €2.20 for 50 GB volume at €0.044/GB = **about €21.89/month before tax**. Recheck volume/IP prices at checkout; the volume/IP figures come from the provider's published cloud page and can change. Add domain renewal, independent backup storage and any paid CDN/WAF. Neither load balancer nor managed database is included in this estimate. [Backup billing](https://docs.hetzner.com/cloud/billing/faq/), [published cloud options](https://www.hetzner.com/de/cloud/?country=en).

Hetzner's cloud product provides VM, network, firewall, storage and Kubernetes integration building blocks; this implementation uses those VMs, not a Hetzner-managed Kubernetes control plane. Third-party managed Kubernetes offerings on Hetzner are separate services with their own charges/support boundaries. Existing Helm deployment remains available for an operator-managed cluster. [Hetzner cloud product](https://www.hetzner.com/cloud/), [cloud server overview](https://docs.hetzner.com/cloud/servers/overview/).

An EC2 + EBS host can run the same application, but price the chosen region and x86 instance, IPv4, EBS snapshots, outbound traffic, and any NAT/load balancer separately. RDS/ElastiCache can improve operational separation but add persistent baseline cost. Lightsail is the simpler AWS comparison at this scale. Kubernetes is useful if a cluster already exists or multiple-host availability is required; operating a fresh cluster adds maintenance and resource overhead without changing user-facing functionality.

Hetzner server backups exclude attached volumes. The Terraform bootstrap therefore schedules cold application exports onto the root disk, then retains seven exports; they must also be copied off-host. See [backup exclusions](https://docs.hetzner.com/cloud/servers/backups-snapshots/overview/) and [Terraform operations](../deploy/terraform/README.md).

## Login provider options

| Option | Benefits | Operational cost / decision |
| --- | --- | --- |
| Local Argon2 accounts + operator invitations | Offline/self-hosted, simple private recovery | Implemented. Host operator distributes invitation/reset links; no email dependency. |
| GitHub OAuth | Familiar to programming users; stable provider user ID | Requires registering an OAuth app, client secret, callbacks and external availability. Defer; request no repository permissions. Private email access requires `user:email`. |
| Google OpenID Connect | Familiar login; signed identity tokens | Requires client registration, consent configuration and external availability. Defer; identity-only scopes suffice. |
| Generic OIDC with a self-hosted provider | Operator controls identities; can reuse an existing Keycloak/Authentik deployment | Additional service, TLS, backups and update responsibility. Best later option when an identity provider already exists. |

[GitHub OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps), [Google OIDC](https://developers.google.com/identity/openid-connect/openid-connect), [OIDC core](https://openid.net/specs/openid-connect-core-1_0.html).

A future implementation should use authorization-code flow with PKCE and state; validate issuer, audience, expiry and nonce for OIDC. Bind external identities to `(issuer, subject)`, never automatically merge by username or email. Preserve invitations and local recovery, require authenticated account linking, and keep administrator promotion host-only. Do not turn successful provider authentication into unrestricted account creation. These flows are researched, not implemented in this changeset.

## Anubis and CAPTCHA assessment

Anubis sits between a TLS reverse proxy and the protected application and uses policy-driven browser challenges. Upstream explicitly cautions that long-lived WebSocket applications may be a poor fit. This service uses JSON APIs and LSP WebSockets, so placing a blanket challenge in front of every request risks replacing JSON with HTML, breaking editor upgrades, and making API clients unusable. The Anubis documentation endpoint itself returned its challenge page during this research; that is not an end-to-end compatibility test. [Upstream installation](https://github.com/TecharoHQ/anubis/blob/main/docs/docs/admin/installation.mdx), [policy configuration](https://github.com/TecharoHQ/anubis/blob/main/docs/docs/admin/policies.mdx).

Decision: keep Anubis out of the shipped default path. If evaluating it later, pin a release/digest on an isolated staging origin, challenge only appropriate browser navigation, expose its required challenge asset endpoints, and retain the application quotas on exempt API paths. Validate direct JSON access, login/invitation POST, strict Origin/CSRF handling, disabled cookies/JavaScript, accessibility, completion WebSocket initialization and reconnect after expiry. Exempting APIs means Anubis does not protect those APIs against scraping; their IP budgets and edge WAF must remain effective. No Anubis runtime, WebSocket compatibility, or load test was performed here.

Cloudflare Turnstile is a narrower optional alternative for an eventual unrestricted registration flow: challenge the form, validate the token server-side with a short timeout, check hostname/action, and fail closed. Client-side success alone is insufficient; tokens expire after five minutes and are single use. A server secret must be generated/stored outside Terraform and exposed only to the API. This implementation does not ship Turnstile because invitation-only registration already requires an operator-issued bearer capability and preserves self-hosting without an external dependency. [Server-side validation](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/).

Use provider-level network mitigation and an optional CDN/WAF for public deployments. Application limits, invitation gating and challenges constrain work at the origin; they cannot prevent upstream bandwidth saturation. Do not cache authenticated API traffic or request/challenge token URLs. Disable query-string logging for editor tickets and avoid collecting source code or password payloads in security telemetry.
