#!/usr/bin/env bash
set -euo pipefail

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64)
    BICEP_PLATFORM="linux-x64"
    YQ_ARCH="amd64"
    ;;
  aarch64|arm64)
    BICEP_PLATFORM="linux-arm64"
    YQ_ARCH="arm64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

mkdir -p "$HOME/.local/bin"
if ! grep -q 'export PATH="$HOME/.local/bin:$PATH"' "$HOME/.bashrc"; then
  echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
fi
export PATH="$HOME/.local/bin:$PATH"

# Radius CLI (rad)
if ! command -v rad >/dev/null 2>&1; then
  wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
fi
if [[ -x "$HOME/.rad/bin/rad" ]]; then
  ln -sf "$HOME/.rad/bin/rad" "$HOME/.local/bin/rad"
fi

# yq for YAML processing used in scripting/tutorials
if ! command -v yq >/dev/null 2>&1; then
  curl -fsSL "https://github.com/mikefarah/yq/releases/latest/download/yq_linux_${YQ_ARCH}" -o "$HOME/.local/bin/yq"
  chmod +x "$HOME/.local/bin/yq"
fi

# k3d for local k3s workflows (only when Docker is available)
if command -v docker >/dev/null 2>&1; then
  if ! command -v k3d >/dev/null 2>&1; then
    curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
  fi
else
  echo "Skipping k3d install: docker CLI/socket not available in this container runtime."
fi

# Bicep CLI via Azure CLI (arch-aware target)
if command -v az >/dev/null 2>&1; then
  az bicep install --target-platform "$BICEP_PLATFORM"
  if [[ -x "$HOME/.azure/bin/bicep" ]]; then
    ln -sf "$HOME/.azure/bin/bicep" "$HOME/.local/bin/bicep"
  fi
fi

echo "Installed tool versions:"
for cmd in az kubectl helm rad bicep k3d rustc cargo jq yq pwsh; do
  if command -v "$cmd" >/dev/null 2>&1; then
    "$cmd" --version 2>/dev/null | head -n 1 || true
  else
    echo "$cmd: not found"
  fi
done
