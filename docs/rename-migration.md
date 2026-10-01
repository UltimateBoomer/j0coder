# Migrating Locoder installations to j0coder

The rename uses a maintenance cutover. Keep the public origin, credentials,
database schema, `practice` database/user, `practice_session` cookie, queue keys,
and browser solution-draft keys unchanged. Accounts, submissions, drafts, and
catalog revisions require no schema migration. Browser theme and blind-mode
preferences are copied from `locoder:*` only when their `j0coder:*` key is absent;
the old values remain available for rollback on the same browser origin.

Build the new application and toolchain images before stopping an installation.
They must be deployed together: sandbox labels and cleanup selectors, the
`J0CODER_STREAM_PROTOCOL` flag, and `j0coder-kotlin-lsp` launcher all changed.
Replace monitoring queries for `locoder_submissions`, `locoder_executions_total`,
`locoder_execution_seconds_total`, and `locoder_editor_sessions` with their
`j0coder_` equivalents. No mixed-version rolling deployment is supported.

## Compose and native development

1. Record the old Git revision, image references, Compose file, and configuration.
   Stop accepting submissions and let active jobs finish. Stop native services
   with the **old revision's** `make dev-down`, or stop the old full Compose stack.
   Before switching sandbox labels, remove only leftover containers bearing
   `locoder.editor=true` or `locoder.sandbox=true`; preserve database volumes.
   Do not use `compose down --volumes`.
2. Use the **pre-rename revision's** ordinary backup tools to export the old
   `locoder_postgres` and `locoder_valkey` volumes and preserve `.env` and
   `data/config`. For the full Compose stack, its existing backup command is:

   ```sh
   scripts/backup.sh "$HOME/backups/locoder-before-j0coder"
   ```

   For native development, stop the native processes first and select the
   dependency stack with `COMPOSE_FILE=compose.dev.yaml`. The existing backup
   script restarts its Compose services when finished; stop them again before
   starting j0coder. The backup format is unchanged. There are no rename-specific
   upgrade scripts or command-line options.
3. Clone the renamed app into a fresh sibling `j0coder` directory and the catalog
   into `j0coder-problems`. Restore from the app checkout before configuring it:

   ```sh
   scripts/restore.sh "$HOME/backups/locoder-before-j0coder"
   ```

   Restore requires a fresh checkout without `.env` and refuses existing
   destination volumes. Old backups work directly. It creates `j0coder_postgres`
   and `j0coder_valkey` and preserves the old volumes. Never run both stacks
   against the same host ports.
4. Review the restored `.env` without printing its credentials. Change the old
   socket basename to `j0coder-podman.sock`, update absolute checkout/runtime and
   SSH paths, and select the rebuilt images. Keep passwords and public origin.
   Update the stored catalog setting `repository_url` through the administrator
   catalog API/UI to `git@github.com:UltimateBoomer/j0coder-problems.git`, preserving
   its strategy, revision, and other settings. `CATALOG_REPOSITORY_URL` is the
   corresponding environment setting for initial configuration only; it does
   not overwrite catalog settings already stored in PostgreSQL.
5. Run `make dev-up` for native development. For the full stack, run
   `make dev-gvisor`, `make images`, `scripts/gvisor-controller.sh start`, and
   `make up` in that order. Check `/readyz`, login, saved drafts and submissions, catalog
   reconciliation, execution in every supported language, and editor completion.
   Verify new sandbox resources are created and cleaned up. Keep the old checkout,
   archives, and volumes until acceptance passes.

## Kubernetes and Lima

Save the old Helm values, rendered resources, replicas, image digests, and ingress
configuration. Stop new submissions, drain execution, and stop the old API,
worker, editor, and catalog deployments. Back up external PostgreSQL and Valkey
using their existing backup procedures. Remove old transient sandbox Pods after
they finish; keep persistent services and their data.

Create the `j0coder` application namespace and `j0coder-sandbox` namespace. The
sandbox namespace is chart-owned; if creating it before installation to stage
Secrets, set its Helm ownership metadata before installing the release:

```sh
kubectl label namespace j0coder-sandbox app.kubernetes.io/managed-by=Helm --overwrite
kubectl annotate namespace j0coder-sandbox \
  meta.helm.sh/release-name=j0coder meta.helm.sh/release-namespace=j0coder --overwrite
```

Copy
referenced database, queue, catalog SSH, TLS, and private-image pull Secrets into
the namespaces where they are used. Configure the new chart with the **same**
external database and queue connections, rebuilt image digests, original public
origin, and renamed catalog URL. Install release `j0coder` from
`deploy/helm/j0coder` with ingress disabled first. There must be only one active
set of consumers/catalog controllers. After readiness and execution checks,
remove the old ingress and enable the new ingress. Retain the old stopped release
and its saved manifests for rollback; do not try to rename a Helm release in place.

Lima development databases use `emptyDir` storage. Export any needed database and
queue data before creating the separate `j0coder` VM; do not delete the `locoder`
VM until verification completes. The new default does not adopt the old VM.
Existing project-local gVisor installations are reusable: the installer accepts
the old `.locoder-patch-hash` marker and writes `.j0coder-patch-hash`.

## GitHub and release coordination

Rename the existing repositories to `UltimateBoomer/j0coder` and
`UltimateBoomer/j0coder-problems`, and update each clone's `origin`. Do not create
new repositories under the old names: that would displace GitHub's redirects.
Update catalog CI to check out both renamed repositories, run `cargo -p j0coder`,
and use the `j0coder-sandbox` self-hosted runner label. Add that label to existing
runners before switching the workflow. Configure `J0CODER_READ_TOKEN` with a
read-only app-repository token if the app is private; keep the existing required
`Catalog / validate` check name. Repository rename does not provision a runner or
grant the catalog's `github.token` access to a different private repository.

Publish a new GitHub release only after both changesets pass review and checks.
The release workflow publishes `ghcr.io/ultimateboomer/j0coder-app` and
`ghcr.io/ultimateboomer/j0coder-toolchain`. Verify package association and actual
deployment-account pulls, then use the workflow's immutable digests. Keep old
image packages for rollback and update external deploy-key, webhook, dashboard,
runner, or machine configuration references that contain the old repository name.

## Rollback

Stop j0coder first, including its sandbox containers/Pods and catalog controller.
If the new installation received no writes, restart the old revision with its old
configuration and original volumes/services. If it received writes, take a new
consistent backup. Manually create `locoder-rollback_postgres` and
`locoder-rollback_valkey` volumes with Podman, import the latest archives, and run
the old Compose file with `-p locoder-rollback`. Refuse any pre-existing rollback
volumes; never overwrite the original volumes or resume stale database/queue
copies. For Kubernetes, retain the current external data and
restore the old workload and ingress configuration. Keep paired old app/toolchain
images; the rename introduces no database changes to reverse.
