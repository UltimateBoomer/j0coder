terraform {
  required_version = ">= 1.9, < 2.0"
  required_providers {
    hcloud = { source = "hetznercloud/hcloud", version = ">= 1.67.0, < 2.0" }
  }
}
# HCLOUD_TOKEN comes from the environment. No secret input variables.
provider "hcloud" {}
variable "name" {
  type    = string
  default = "j0coder"
}
variable "domain" {
  type = string
}
variable "release_version" {
  type = string
}
variable "ssh_public_keys" {
  type = list(string)
}
variable "location" {
  type    = string
  default = "fsn1"
}
variable "ipv6_enabled" {
  type    = bool
  default = true
}
variable "guest_browsing" {
  type    = bool
  default = false
}
variable "web_admin" {
  type    = bool
  default = false
}
variable "registration_mode" {
  type    = string
  default = "invite"
}
variable "operator_cidrs" {
  type = list(string)
  validation {
    condition     = length(var.operator_cidrs) > 0 && alltrue([for cidr in var.operator_cidrs : can(cidrhost(cidr, 0)) && !endswith(cidr, "/0")])
    error_message = "Supply explicit SSH operator CIDRs. Unrestricted /0 access is forbidden."
  }
}
variable "server_type" {
  type    = string
  default = "cx43"
  validation {
    condition     = can(regex("^(cx|cpx|ccx)[0-9]+$", var.server_type))
    error_message = "Use an x86_64 CX, CPX, or CCX type. ARM CAX is unsupported."
  }
}
variable "data_size_gb" {
  type    = number
  default = 50
  validation {
    condition     = var.data_size_gb >= 50 && floor(var.data_size_gb) == var.data_size_gb
    error_message = "Application storage must be an integer of at least 50 GB."
  }
}
resource "hcloud_ssh_key" "operator" {
  for_each   = { for i, key in var.ssh_public_keys : tostring(i) => key }
  name       = "${var.name}-operator-${each.key}"
  public_key = each.value
}
resource "hcloud_firewall" "host" {
  name = "${var.name}-ingress"
  rule {
    direction  = "in"
    protocol   = "tcp"
    port       = "22"
    source_ips = var.operator_cidrs
  }
  rule {
    direction  = "in"
    protocol   = "tcp"
    port       = "80"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
  rule {
    direction  = "in"
    protocol   = "tcp"
    port       = "443"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
  rule {
    direction  = "in"
    protocol   = "icmp"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
}
resource "hcloud_volume" "data" {
  name              = "${var.name}-data"
  size              = var.data_size_gb
  location          = var.location
  format            = "ext4"
  delete_protection = true
  lifecycle {
    prevent_destroy = true
  }
}
resource "hcloud_primary_ip" "ipv4" {
  name        = "${var.name}-ipv4"
  location    = var.location
  type        = "ipv4"
  auto_delete = false
  lifecycle {
    prevent_destroy = true
  }
}
module "bootstrap" {
  source            = "../modules/host-bootstrap"
  domain            = var.domain
  release_version   = var.release_version
  ssh_public_keys   = var.ssh_public_keys
  data_device       = hcloud_volume.data.linux_device
  guest_browsing    = var.guest_browsing
  web_admin         = var.web_admin
  registration_mode = var.registration_mode
}
resource "hcloud_server" "host" {
  name               = var.name
  image              = "ubuntu-24.04"
  server_type        = var.server_type
  location           = var.location
  ssh_keys           = [for key in hcloud_ssh_key.operator : key.id]
  firewall_ids       = [hcloud_firewall.host.id]
  user_data          = module.bootstrap.user_data
  backups            = true
  delete_protection  = true
  rebuild_protection = true
  public_net {
    ipv4_enabled = true
    ipv4         = hcloud_primary_ip.ipv4.id
    ipv6_enabled = var.ipv6_enabled
  }
  lifecycle {
    prevent_destroy = true
    ignore_changes  = [user_data]
  }
}
resource "hcloud_volume_attachment" "data" {
  volume_id = hcloud_volume.data.id
  server_id = hcloud_server.host.id
  automount = false
}
output "ipv4" {
  value = hcloud_primary_ip.ipv4.ip_address
}
output "ipv6" {
  value = var.ipv6_enabled ? hcloud_server.host.ipv6_address : null
}
output "onboarding" {
  value = "ssh operator@${hcloud_primary_ip.ipv4.ip_address}, then sudo cloud-init status --wait and sudo /usr/local/sbin/j0coder-onboard"
}
