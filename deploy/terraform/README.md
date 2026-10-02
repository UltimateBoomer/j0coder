# Single-host deployment

`hetzner/` creates one x86_64 Ubuntu 24.04 host, a protected primary IPv4, a protected 50 GB ext4 volume, provider server backups, and an ingress firewall (operator SSH CIDRs, public HTTP/HTTPS and ICMP). The application, database, Valkey and sandbox controllers use the existing rootless Podman deployment. No Kubernetes cluster or AWS-specific resources are created.

`modules/host-bootstrap/` is reusable cloud-init for another provider's Ubuntu host. Pass a stable, already formatted ext4 data-device path, domain, pinned release tag and public SSH keys. Your provider template must attach that device and configure firewall, addressing and backup protection. It is a bootstrap module, not a generic provider-independent VM resource.

## Provision and onboard

1. Copy `hetzner/terraform.tfvars.example` to an ignored `terraform.tfvars`; supply your real DNS hostname, release tag, public SSH key, and explicit operator CIDRs. Select a tag that includes these security changes and matching release assets. An older release will not acquire these features from Terraform.
2. Set `HCLOUD_TOKEN` in the environment, then run `terraform init`, `terraform plan`, review the plan, and `terraform apply` in `hetzner/`. Use a protected state backend for a real installation; local state is ignored. Cloud-init contains no account password, database secret, invitation or reset token. State still contains host metadata and public keys.
3. Point DNS A to the IPv4 output. Publish AAAA only if IPv6 is enabled and routed. SSH as `operator`; wait for `cloud-init status --wait`. The mount check prevents application startup without the intended volume. If an attachment took longer than three minutes, rerun `sudo /usr/local/sbin/j0coder-host-prepare` after investigating cloud-init logs.
4. Run `sudo /usr/local/sbin/j0coder-onboard` over the SSH terminal. It downloads the installer at the pinned tag, prompts for the initial local administrator password without echo, generates application secrets on-host, starts the service and waits for readiness before enabling Caddy and the backup timer. The installer verifies release archives through its release manifest. No application passwords are Terraform inputs.
5. Check the HTTPS origin, sign-in, invitation redemption, one sandbox run, semantic completion, settings persistence and a restore on a separate host. Verify the public admin API is 404 and no API/editor/database/queue ports are reachable externally.

Release updates and feature changes do not replace the VM: Terraform ignores later `user_data` changes. Apply application updates using the installer over SSH, and edit non-secret feature settings in `/home/j0coder/.local/share/j0coder/.env` followed by `scripts/setup.sh up`. Do not assume changing a Terraform flag configures an existing host.

The VM, primary IPv4 and data volume use deletion protection and `prevent_destroy`. A deliberate teardown requires removing those protections; ordinary `terraform destroy` is refused. Volume size can grow, but filesystem growth is a separate operator step. Never resize storage smaller.

## Backup and recovery

Daily 04:00 UTC backups call the existing cold backup script: all Compose services pause briefly while PostgreSQL/Valkey volumes, configuration and secret environment are exported. The newest seven directories survive in `/home/j0coder/backups` on the server's root disk, with private permissions. This timer is enabled only after onboarding succeeds.

[Hetzner server backups](https://docs.hetzner.com/cloud/servers/backups-snapshots/overview/) do **not** include attached volumes. The cold exports place application data on the root disk so a subsequent server backup can include it; copy these exports to an independent encrypted/off-host destination as well. Disk space must fit seven full exports. Monitor the timer (`systemctl status j0coder-backup.timer`, `journalctl -u j0coder-backup.service`), backup freshness and free space. This is a single host with downtime on failure and an approximately daily recovery point, not high availability.

For recovery, create an isolated fresh host at the same application release, retain the original protected volume/IPv4 until recovery is verified, and use `scripts/restore.sh ABSOLUTE_BACKUP_DIRECTORY` in a fresh installation without `.env` or existing application volumes. Review the restored origin/socket paths and restart the rootless deployment. The script refuses to overwrite an existing installation. Test a real restore before relying on this deployment; Terraform mocked tests do not verify cloud-init, gVisor or disaster recovery.

## Validation

```sh
terraform -chdir=hetzner init -backend=false
terraform -chdir=hetzner validate
terraform -chdir=hetzner test
terraform fmt -check -recursive .
```

The tests use a mock Hetzner provider and never call cloud APIs. They check bootstrap prerequisites and backup/deletion protections, and reject public SSH and ARM server types. Cloud resources were not provisioned during implementation.
