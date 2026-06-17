#!/usr/bin/env bash
#
# Adaptive Apps CLI installer.
#
# Downloads a release archive of `ada` (the Adaptive Apps CLI) from
# https://github.com/microsoft/adaptive-apps/releases, installs the binary
# into a system bin directory (default /usr/local/bin) so it lands on PATH
# automatically, and stages the bundled Radius artifacts under $ADA_HOME
# (default ~/.adaptive).
#
# One-liner:
#   curl -fsSL https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.sh | bash
#
# Environment overrides:
#   ADA_VERSION       Release tag to install (e.g. cli-v0.1.0). Defaults to
#                     the latest cli-v* release.
#   ADA_INSTALL_DIR   Where to drop the `ada` binary (default /usr/local/bin).
#                     Uses sudo automatically when not writable.
#   ADA_HOME          Root for bundled artifacts (default ~/.adaptive).
#                     The Radius tree lands at $ADA_HOME/radius.
#   ADA_REPO          owner/repo to pull from (default microsoft/adaptive-apps).
#
set -euo pipefail

ADA_REPO="${ADA_REPO:-microsoft/adaptive-apps}"
ADA_INSTALL_DIR="${ADA_INSTALL_DIR:-/usr/local/bin}"
ADA_HOME="${ADA_HOME:-$HOME/.adaptive}"
ADA_VERSION="${ADA_VERSION:-}"

ADA_BIN_NAME="ada"
ADA_BIN_PATH="${ADA_INSTALL_DIR}/${ADA_BIN_NAME}"
ADA_HTTP_CLI=""
ADA_TMP_ROOT=""
USE_SUDO="false"

log()  { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m  %s\n' "$*" >&2; }
die()  { printf '\033[1;31mxx\033[0m  %s\n' "$*" >&2; exit 1; }

on_exit() {
  local rc=$?
  if [[ -n "${ADA_TMP_ROOT}" && -d "${ADA_TMP_ROOT}" ]]; then
    rm -rf "${ADA_TMP_ROOT}"
  fi
  if [[ $rc -ne 0 ]]; then
    warn "Adaptive Apps CLI install failed (exit ${rc})."
  fi
  exit $rc
}
trap on_exit EXIT

# ---------------------------------------------------------------------------
# Environment probes
# ---------------------------------------------------------------------------

detect_system() {
  local arch_raw os_raw
  arch_raw="$(uname -m)"
  os_raw="$(uname | tr '[:upper:]' '[:lower:]')"

  case "$os_raw" in
    linux)  OS_PART="unknown-linux-gnu" ;;
    darwin) OS_PART="apple-darwin" ;;
    *) die "unsupported OS '${os_raw}'; use install.ps1 on Windows" ;;
  esac
  case "$arch_raw" in
    x86_64|amd64)  ARCH_PART="x86_64" ;;
    arm64|aarch64) ARCH_PART="aarch64" ;;
    *) die "unsupported architecture '${arch_raw}'" ;;
  esac
  TARGET="${ARCH_PART}-${OS_PART}"
}

check_downloader() {
  if command -v curl >/dev/null 2>&1; then
    ADA_HTTP_CLI=curl
  elif command -v wget >/dev/null 2>&1; then
    ADA_HTTP_CLI=wget
  else
    die "either curl or wget is required to download release artifacts"
  fi
  command -v tar >/dev/null 2>&1 || die "tar is required to extract release archives"
}

decide_sudo() {
  if [[ -d "${ADA_INSTALL_DIR}" && -w "${ADA_INSTALL_DIR}" ]]; then
    USE_SUDO="false"
    return
  fi
  if [[ ! -e "${ADA_INSTALL_DIR}" ]]; then
    local parent
    parent="$(dirname "${ADA_INSTALL_DIR}")"
    if [[ -d "${parent}" && -w "${parent}" ]]; then
      USE_SUDO="false"
      return
    fi
  fi
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    USE_SUDO="false"
    return
  fi
  command -v sudo >/dev/null 2>&1 \
    || die "${ADA_INSTALL_DIR} is not writable and sudo is not available; rerun with ADA_INSTALL_DIR=\$HOME/.local/bin"
  USE_SUDO="true"
}

run_as_root() {
  if [[ "${USE_SUDO}" == "true" ]]; then
    sudo -- "$@"
  else
    "$@"
  fi
}

http_get() {
  local url="$1" out="$2"
  if [[ "${ADA_HTTP_CLI}" == "curl" ]]; then
    curl -fSL --retry 3 -o "${out}" "${url}"
  else
    wget -q -O "${out}" "${url}"
  fi
}

http_get_text() {
  local url="$1"
  if [[ "${ADA_HTTP_CLI}" == "curl" ]]; then
    curl -fsSL -H 'Accept: application/json' "${url}"
  else
    wget -q --header='Accept: application/json' -O - "${url}"
  fi
}

# ---------------------------------------------------------------------------
# Release resolution
# ---------------------------------------------------------------------------

resolve_version() {
  if [[ -n "${ADA_VERSION}" ]]; then
    RELEASE_TAG="${ADA_VERSION}"
    return
  fi
  local body tag
  body="$(http_get_text "https://api.github.com/repos/${ADA_REPO}/releases?per_page=30" || true)"
  tag="$(printf '%s' "$body" \
    | grep -Eo '"tag_name":[[:space:]]*"cli-v[^"]+"' \
    | head -n1 \
    | sed -E 's/.*"(cli-v[^"]+)".*/\1/')"
  [[ -n "$tag" ]] || die "could not resolve latest cli-v* release from ${ADA_REPO}; set ADA_VERSION"
  RELEASE_TAG="$tag"
}

check_existing() {
  if [[ -x "${ADA_BIN_PATH}" ]]; then
    log "Existing install detected: ${ADA_BIN_PATH}"
    "${ADA_BIN_PATH}" --version 2>/dev/null || true
    log "Reinstalling..."
  fi
}

# ---------------------------------------------------------------------------
# Download + install
# ---------------------------------------------------------------------------

download_release() {
  local version="${RELEASE_TAG#cli-v}"
  ASSET_NAME="ada-${version}-${TARGET}.tar.gz"
  local url="https://github.com/${ADA_REPO}/releases/download/${RELEASE_TAG}/${ASSET_NAME}"

  ADA_TMP_ROOT="$(mktemp -d -t ada-install.XXXXXX)"
  ASSET_TMP="${ADA_TMP_ROOT}/${ASSET_NAME}"

  log "Downloading ${url}"
  http_get "${url}" "${ASSET_TMP}" \
    || die "download failed (asset not published?); try ADA_VERSION=cli-vX.Y.Z"
  [[ -f "${ASSET_TMP}" ]] || die "download produced no file at ${ASSET_TMP}"

  log "Extracting archive"
  tar -xzf "${ASSET_TMP}" -C "${ADA_TMP_ROOT}"
  EXTRACTED_DIR="${ADA_TMP_ROOT}/ada-${version}-${TARGET}"
  [[ -d "${EXTRACTED_DIR}" ]]                       || die "archive layout unexpected; missing ${EXTRACTED_DIR}"
  [[ -x "${EXTRACTED_DIR}/bin/${ADA_BIN_NAME}" ]]   || die "archive missing bin/${ADA_BIN_NAME}"
}

install_binary() {
  log "Installing ${ADA_BIN_NAME} to ${ADA_INSTALL_DIR}"
  if [[ ! -d "${ADA_INSTALL_DIR}" ]]; then
    run_as_root mkdir -p "${ADA_INSTALL_DIR}"
  fi
  chmod 0755 "${EXTRACTED_DIR}/bin/${ADA_BIN_NAME}"
  if [[ -f "${ADA_BIN_PATH}" ]]; then
    run_as_root rm -f "${ADA_BIN_PATH}"
  fi
  run_as_root install -m 0755 "${EXTRACTED_DIR}/bin/${ADA_BIN_NAME}" "${ADA_BIN_PATH}"

  [[ -x "${ADA_BIN_PATH}" ]] || die "failed to install ${ADA_BIN_NAME} into ${ADA_INSTALL_DIR}"
}

install_artifacts() {
  mkdir -p "${ADA_HOME}/radius"
  if [[ -d "${EXTRACTED_DIR}/radius" ]]; then
    log "Staging Radius artifacts under ${ADA_HOME}/radius"
    # Replace contents but keep the directory itself so user-mounted
    # symlinks/permissions aren't disturbed.
    rm -rf "${ADA_HOME}/radius"/*
    (cd "${EXTRACTED_DIR}/radius" && tar -cf - .) \
      | (cd "${ADA_HOME}/radius" && tar -xf -)
  else
    warn "release archive did not bundle radius/ — skipping"
  fi
}

welcome() {
  log "Installed $(${ADA_BIN_PATH} --version 2>/dev/null || echo "${ADA_BIN_NAME}")"
  log "Binary  : ${ADA_BIN_PATH}"
  log "Home    : ${ADA_HOME}"
  log "Radius  : ${ADA_HOME}/radius"

  case ":${PATH:-}:" in
    *":${ADA_INSTALL_DIR}:"*) ;;
    *)
      warn "${ADA_INSTALL_DIR} is not on your current PATH."
      warn "Open a new shell, or run: export PATH=\"${ADA_INSTALL_DIR}:\$PATH\""
      ;;
  esac

  cat <<EOF

Get started:
  ada --help
  ada radius path --artifact root --require-exists
  ada bootstrap --portfolio min --platform localk8s

EOF
}

# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

detect_system
check_downloader
decide_sudo
resolve_version

log "Installing Adaptive Apps CLI"
log "  release  : ${RELEASE_TAG}"
log "  target   : ${TARGET}"
log "  bin dir  : ${ADA_INSTALL_DIR}$([[ ${USE_SUDO} == true ]] && echo ' (sudo)')"
log "  home     : ${ADA_HOME}"

check_existing
download_release
install_binary
install_artifacts
welcome
