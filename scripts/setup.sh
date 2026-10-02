#!/bin/sh
set -eu
if ! command -v python3 >/dev/null 2>&1; then
  choice=unset
  for arg in "$@"; do
    case "$arg" in --install-packages) choice=yes ;; --no-install-packages) choice=no ;; esac
  done
  if command -v apt-get >/dev/null 2>&1; then
    echo "Required commands: sudo apt-get update; sudo apt-get install -y python3"
    manager=apt
  elif command -v dnf >/dev/null 2>&1; then
    echo "Required command: sudo dnf install -y python3"
    manager=dnf
  else
    echo 'Install Python 3.9+ with your distribution package manager.' >&2
    exit 1
  fi
  echo 'Python 3.9+ is required to run the setup coordinator.'
  if [ "$choice" = unset ]; then
    printf '\nINPUT REQUIRED — Install Python\nThe sudo commands above install the setup prerequisite. Enter accepts no.\n'
    while :; do
      if ! printf '> Install Python with sudo? [no]: ' > /dev/tty; then
        echo 'Python installation needs /dev/tty; supply --install-packages or install Python manually.' >&2
        exit 1
      fi
      read -r choice < /dev/tty || { echo 'Python installation input closed.' >&2; exit 1; }
      case "$choice" in
        [Yy]|[Yy][Ee][Ss]) choice=yes; break ;;
        ''|[Nn]|[Nn][Oo]) choice=no; break ;;
        *) echo 'Invalid answer; enter yes/y or no/n.' ;;
      esac
    done
  fi
  [ "$choice" = yes ] || { echo 'Python is required; use --install-packages to authorize installation.' >&2; exit 1; }
  if [ "$manager" = apt ]; then
    sudo apt-get update
    sudo apt-get install -y python3
  else
    sudo dnf install -y python3
  fi
fi
exec python3 "$(dirname "$0")/setup.py" "$@"
