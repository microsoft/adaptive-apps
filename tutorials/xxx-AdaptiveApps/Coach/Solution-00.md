# Challenge 00 - Prerequisites - Ready, Set, GO! - Coach's Guide

**[Home](./README.md)** - [Next Solution >](./Solution-01.md)

## Notes & Guidance

Challenge 00 is a prerequisite verification challenge. Students should arrive with most tools already installed or be prepared to install them quickly. Your role as coach is to help unblock any environment issues so the team can move forward into Challenge 01 cluster setup.

## Installation Instructions

Below are step-by-step instructions for installing all required tools on Linux, macOS, and Windows (via WSL2). Share these with students before the hack, or use them during Challenge 00 to help teams get up and running.

### macOS

**Azure CLI:**
```bash
brew install azure-cli
```

**kubectl:**
```bash
brew install kubectl
```

**Helm:**
```bash
brew install helm
```

**Radius CLI:**
```bash
curl -fsSL https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh | /bin/bash
```

If `rad` is not found after install, add the installer path and reload your shell:
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

**VS Code:**
- Download from [https://code.visualstudio.com](https://code.visualstudio.com) and install, or:
```bash
brew install --cask visual-studio-code
```

**Verify installation:**
```bash
az --version
kubectl version --client
helm version
rad version
code --version
```

### Linux (Ubuntu/Debian)

**Azure CLI:**
```bash
curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash
```

**kubectl:**
```bash
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl
```

**Helm:**
```bash
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

**Radius CLI:**

Use the official Radius installation script (recommended):

```bash
wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
```

After installation, add Radius to your PATH:
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

**Verify installation:**
```bash
rad version
```

If the script fails, manually download the binary from [https://github.com/radius-project/radius/releases](https://github.com/radius-project/radius/releases):
```bash
# Download rad_linux_amd64 or rad_linux_arm64 for your architecture
chmod +x rad_linux_*
mkdir -p ~/.local/bin
mv rad_linux_* ~/.local/bin/rad
# Then add ~/.local/bin to PATH as shown above
```

**VS Code:**
- Download from [https://code.visualstudio.com](https://code.visualstudio.com), or:
```bash
sudo snap install code --classic
```

**Verify installation:**
```bash
az --version
kubectl version --client
helm version
rad version
code --version
```

### Windows (WSL2 + PowerShell 7)

First, ensure you have WSL2 installed and a Linux distribution (Ubuntu recommended):
```powershell
wsl --install -d Ubuntu
```

**Option A: Use WSL2 bash for tools**

Open your WSL2 terminal and run the Linux installation commands above.

**Option B: Use PowerShell 7 on Windows with Windows package managers**

Install PowerShell 7 (if not already installed):
```powershell
iex "& { $(irm https://aka.ms/install-powershell.ps1) } -UseMSI"
```

Then install tools via winget:

**Azure CLI:**
```powershell
winget install Microsoft.AzureCLI
```

**kubectl:**
```powershell
winget install Kubernetes.kubectl
```

**Helm:**
```powershell
winget install Helm.Helm
```

**Radius CLI:**
```powershell
winget install RadiusProject.rad
```

**VS Code:**
- Download from [https://code.visualstudio.com](https://code.visualstudio.com) and run installer

**Verify installation (PowerShell):**
```powershell
az --version
kubectl version --client
helm version
rad version
code --version
```

**Note on WSL2:** For Challenge 01, you may need to run the Azure CLI and kubectl commands from both Windows PowerShell *and* WSL2 bash (depending on where your cluster is). Windows PowerShell is fine for Azure resource creation; WSL2 bash works better for connecting to local clusters (kind, k3d) or Arc proxy scenarios.

### Troubleshooting Installation

| Tool | Issue | Solution |
|------|-------|----------|
| az | Command not found | Verify installation completed; restart terminal; check PATH |
| kubectl | Cannot connect to cluster | Ensure kubeconfig is in `~/.kube/config`; run `kubectl config view` |
| helm | Permission denied | Use `sudo helm` or check file permissions in `~/.helm` |
| rad | Old version | Download latest from GitHub releases; reinstall |
| VS Code | Extensions not loading | Try `code --install-extension ms-azure-tools.vscode-azuretools` |

### Verification Checklist

Use these commands to verify each student's environment before they proceed to Challenge 01:

**Azure Subscription & CLI:**
- `az account show` → confirms Azure CLI is installed and signed in
- `az --version` → shows CLI version (should be recent)
- Verify Owner access to at least one subscription where they can create resource groups

**kubectl:**
- `kubectl version --client` → confirms kubectl is installed (version 1.24+)
- They may not have a cluster connected yet; that's Challenge 01's job

**Helm:**
- `helm version` → confirms Helm is installed (3.12+)
- Students will need this for Challenge 02 onward

**Radius CLI (rad):**
- `rad version` → confirms Radius CLI is installed (latest stable version)
- If missing, point them to [https://docs.radapp.io/getting-started/install/](https://docs.radapp.io/getting-started/install/)

**VS Code:**
- Just verify they can open it and have basic extensions (Azure Account, Kubernetes, REST Client optional)

**Shell:**
- On Windows: PowerShell 7+ or WSL2 bash. On Mac/Linux: bash or zsh
- They'll need this for running the prep scripts in Challenge 01

### Common Issues

**"az: command not found"** → Azure CLI not installed. Direct to https://learn.microsoft.com/cli/azure/install-azure-cli

**"kubectl: command not found"** → kubectl not in PATH. Verify installation and PATH environment variable

**"rad: command not found"** → Radius CLI not installed. Direct to Radius docs or check if they're using a container-based approach

**"Permission denied" on az commands** → Likely not signed in. Run `az login` and follow browser flow

**Multiple subscriptions but wrong one selected** → Use `az account set --subscription <id>` to set default

### Tips for Coaches

- **Send installation instructions early:** Share the Installation Instructions section (above) with students 1-2 weeks before the hack so they have time to install locally. Prepare different instructions for students on different OSes.
- **Pre-hack checklist email:** Create a simple email template asking students to run the Verification Checklist commands and report back. Students who come prepared will move faster through Challenge 01.
- **Fallback for delays:** If students arrive without tools, have them use Azure Cloud Shell (browser-based bash with az CLI pre-installed) as a temporary fallback. They can provision Azure resources from Cloud Shell, then configure kubectl from their local machine.
- **Check Azure quotas early:** Before the hack, verify the team's Azure subscription has sufficient quota for AKS (cores, IPs, etc.) — insufficient quota often blocks Challenge 01 entirely. Request increases if needed.
- **Remind priority:** Challenge 01 is where the real platform engineering work begins; Challenge 00 is just verification. Don't let teams get bogged down troubleshooting obscure environment issues here — help them move forward.

### What Students Should Know After Challenge 00

After completing this challenge, students should be able to:
- Use Azure CLI to list and manage Azure resources
- Understand the basics of kubectl and how to connect to clusters
- Know where to find the Radius documentation
- Understand their development environment and any shell limitations

All of these are required to successfully navigate **Challenge 01: Prepare the Platforms**, where they'll either use the provided automation script ([prepare-azure-local.ps1](../../common/prepare-azure-local.ps1)) or follow the manual platform prep guides ([prepare-aks.md](../../common/prepare-aks.md), [prepare-azure-local.md](../../common/prepare-azure-local.md), etc.).
