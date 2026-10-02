terraform {
  required_version = ">= 1.9, < 2.0"
}
variable "domain" {
  type = string
  validation {
    condition     = can(regex("^[a-z0-9]([a-z0-9.-]*[a-z0-9])?\\.[a-z]{2,}$", var.domain)) && !strcontains(var.domain, "..")
    error_message = "Use a lowercase DNS hostname without a scheme, port, wildcard, or trailing dot."
  }
}
variable "release_version" {
  type = string
  validation {
    condition     = can(regex("^v[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.-]+)?$", var.release_version))
    error_message = "Pin a complete release tag. Latest is not accepted."
  }
}
variable "ssh_public_keys" {
  type = list(string)
  validation {
    condition     = length(var.ssh_public_keys) > 0 && alltrue([for key in var.ssh_public_keys : can(regex("^ssh-(ed25519|rsa) [A-Za-z0-9+/=]+( [^\\r\\n]+)?$", key))])
    error_message = "Supply single-line public keys, never private keys."
  }
}
variable "data_device" {
  type = string
  validation {
    condition     = can(regex("^/dev/disk/by-id/[A-Za-z0-9_-]+$", var.data_device))
    error_message = "Supply a stable /dev/disk/by-id path."
  }
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
  validation {
    condition     = contains(["invite", "closed"], var.registration_mode)
    error_message = "Registration must be invite or closed."
  }
}
locals {
  config = {
    users = ["default", {
      name                = "operator"
      groups              = "sudo"
      shell               = "/bin/bash"
      sudo                = "ALL=(ALL) NOPASSWD:ALL"
      lock_passwd         = true
      ssh_authorized_keys = var.ssh_public_keys
      }, {
      name        = "j0coder"
      shell       = "/bin/bash"
      lock_passwd = true
    }]
    ssh_pwauth     = false
    disable_root   = true
    package_update = true
    packages       = ["make", "podman", "podman-compose", "uidmap", "dbus-user-session", "slirp4netns", "fuse-overlayfs", "python3", "curl", "ca-certificates", "caddy", "jq"]
    write_files = [{
      path        = "/usr/local/sbin/j0coder-host-prepare"
      permissions = "0755"
      content = templatefile("${path.module}/prepare.sh.tftpl", {
        domain       = var.domain
        version      = var.release_version
        device       = var.data_device
        guest        = var.guest_browsing ? "true" : "false"
        admin        = var.web_admin ? "true" : "false"
        registration = var.registration_mode
      })
    }]
    runcmd = [["/usr/local/sbin/j0coder-host-prepare"]]
  }
}
output "user_data" {
  value = "#cloud-config\n${yamlencode(local.config)}"
}
