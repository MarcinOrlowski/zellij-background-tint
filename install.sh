#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_NAME SCRIPT_DIR

readonly TARGET="wasm32-wasip1"
readonly CRATE="zellij-background-tint"

log_info() { printf '%s\n' "${1}"; }
log_error() { printf '%s\n' "ERROR: ${1}" >&2; }

show_usage() {
  cat <<EOF
Usage: ${SCRIPT_NAME} [-h]

Build the ${CRATE} Zellij plugin for ${TARGET} and install the resulting
.wasm into your Zellij plugins directory.

  -h    Show this help

Environment:
  XDG_CONFIG_HOME   Base config dir (default: \${HOME}/.config)
EOF
}

# True if the wasm std for TARGET is available to the active toolchain.
target_ready() {
  if command -v rustup >/dev/null 2>&1; then
    rustup target list --installed 2>/dev/null | grep -qx "${TARGET}"
  else
    # Distro / standalone rustc: look for the target's std in the sysroot.
    local sysroot
    sysroot="$(rustc --print sysroot 2>/dev/null)" || return 1
    [[ -d "${sysroot}/lib/rustlib/${TARGET}" ]]
  fi
}

# Print actionable guidance when the target std is missing, then exit.
fail_missing_target() {
  log_error "Rust target '${TARGET}' is not available to the active toolchain."
  if command -v rustup >/dev/null 2>&1; then
    log_error "Add it with: rustup target add ${TARGET}"
  else
    log_error "The distro Rust toolchain ships no ${TARGET} std, and only rustup can add one."
    log_error "Fix (Ubuntu): sudo apt install rustup && rustup default stable && rustup target add ${TARGET}"
    log_error "Or build in a container without touching the host:"
    log_error "  docker run --rm -v \"\${PWD}\":/w -w /w rust:latest \\"
    log_error "    bash -c 'rustup target add ${TARGET} && cargo build --release --target ${TARGET}'"
  fi
  exit 1
}

main() {
  while getopts ":h" opt; do
    case "${opt}" in
      h)
        show_usage
        exit 0
        ;;
      \?)
        log_error "Unknown option: -${OPTARG}"
        show_usage >&2
        exit 2
        ;;
    esac
  done
  shift $((OPTIND - 1))

  if ! command -v cargo >/dev/null 2>&1; then
    log_error "cargo not found. Install Rust (Ubuntu): sudo apt install rustup && rustup default stable"
    log_error "Or see https://rustup.rs"
    exit 1
  fi

  target_ready || fail_missing_target

  local install_dir artifact
  install_dir="${XDG_CONFIG_HOME:-${HOME}/.config}/zellij/plugins"
  artifact="${SCRIPT_DIR}/target/${TARGET}/release/${CRATE}.wasm"

  log_info "Building ${CRATE} for ${TARGET} ..."
  cargo build --manifest-path "${SCRIPT_DIR}/Cargo.toml" --release --target "${TARGET}"

  if [[ ! -f "${artifact}" ]]; then
    log_error "Build reported success but artifact is missing: ${artifact}"
    exit 1
  fi

  install -d "${install_dir}"
  install -m 0644 "${artifact}" "${install_dir}/${CRATE}.wasm"

  log_info "Installed ${install_dir}/${CRATE}.wasm"
  log_info "See README.md for the Zellij configuration entries."
}

main "$@"
