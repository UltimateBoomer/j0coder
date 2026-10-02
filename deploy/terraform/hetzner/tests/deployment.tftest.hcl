mock_provider "hcloud" {
  mock_resource "hcloud_server" { defaults = { id = "23456" } }
  mock_resource "hcloud_primary_ip" { defaults = { id = "34567" } }
  mock_resource "hcloud_ssh_key" { defaults = { id = "45678" } }
  mock_resource "hcloud_firewall" { defaults = { id = "56789" } }
  mock_resource "hcloud_volume" {
    override_during = plan
    defaults        = { linux_device = "/dev/disk/by-id/scsi-0HC_Volume_12345" }
  }
}
override_resource {
  target          = hcloud_volume.data
  values          = { id = "12345", linux_device = "/dev/disk/by-id/scsi-0HC_Volume_12345" }
  override_during = plan
}
override_resource {
  target          = hcloud_server.host
  values          = { id = "23456" }
  override_during = plan
}
variables {
  domain          = "practice.example.com"
  release_version = "v0.2.0"
  ssh_public_keys = ["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPublicTestKeyOnly"]
  operator_cidrs  = ["192.0.2.1/32"]
}
run "protected_single_host" {
  command = plan
  assert {
    condition     = hcloud_server.host.backups && hcloud_server.host.delete_protection && hcloud_volume.data.delete_protection
    error_message = "Backups and deletion protection must be enabled."
  }
  assert {
    condition     = strcontains(module.bootstrap.user_data, "j0coder-onboard") && !strcontains(module.bootstrap.user_data, "ADMIN_PASSWORD")
    error_message = "Bootstrap must expose SSH onboarding without administrator credentials."
  }
}
run "installer_prerequisites" {
  command = plan
  assert {
    condition     = alltrue([for package in ["make", "podman", "uidmap", "dbus-user-session"] : contains(yamldecode(trimprefix(module.bootstrap.user_data, "#cloud-config\n")).packages, package)])
    error_message = "Bootstrap must install the tools required by the no-install-packages onboarding path."
  }
}
run "reject_public_ssh" {
  command = plan
  variables { operator_cidrs = ["0.0.0.0/0"] }
  expect_failures = [var.operator_cidrs]
}
run "reject_arm" {
  command = plan
  variables { server_type = "cax31" }
  expect_failures = [var.server_type]
}
