#!/bin/sh
# Download and inspect this file before running if preferred.
set -eu
if ! command -v python3 >/dev/null 2>&1; then
  choice=unset
  for arg in "$@"; do
    case "$arg" in --install-packages) choice=yes ;; --no-install-packages) choice=no ;; esac
  done
  if command -v apt-get >/dev/null 2>&1; then
    echo 'Required commands: sudo apt-get update; sudo apt-get install -y python3'
    manager=apt
  elif command -v dnf >/dev/null 2>&1; then
    echo 'Required command: sudo dnf install -y python3'
    manager=dnf
  else
    echo 'Install Python 3.9+ with your distribution package manager.' >&2
    exit 1
  fi
  if [ "$choice" = unset ] && ( : < /dev/tty ) 2>/dev/null; then
    printf 'Install Python with sudo? [yes/no]: ' > /dev/tty
    read -r choice < /dev/tty || choice=no
  fi
  [ "$choice" = yes ] || { echo 'Python is required; use --install-packages to authorize installation.' >&2; exit 1; }
  if [ "$manager" = apt ]; then
    sudo apt-get update
    sudo apt-get install -y python3
  else
    sudo dnf install -y python3
  fi
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
# The small coordinator is fetched from the same public source as this entry point.
curl -fsSL https://raw.githubusercontent.com/UltimateBoomer/j0coder/main/scripts/setup.py -o "$tmp/setup.py"
python3 "$tmp/setup.py" install "$@"
