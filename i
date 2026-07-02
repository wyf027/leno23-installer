#!/bin/sh
set -eu

repo_url="${LENO23_INSTALLER_REPO_URL:-https://github.com/wyf027/leno23-installer}"
bin_name="leno23-install"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required to install ${bin_name}" >&2
  echo "install Rust from https://rustup.rs, then run this script again" >&2
  exit 1
fi

echo "Installing ${bin_name} from ${repo_url} ..."
cargo install --git "${repo_url}" --locked --force

if [ "$#" -eq 0 ]; then
  set -- .
fi

echo
echo "Running ${bin_name} $*"
"${bin_name}" "$@"
