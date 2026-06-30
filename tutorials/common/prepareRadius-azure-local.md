# Prepare Radius on Azure Local

## Prerequisites

- Healthy Azure Local cluster from [prepare-azure-local.md](./prepare-azure-local.md).
- **Direct network access to the Kubernetes API server** (typically 10.x.x.x:6443 on your management VM).
- `cluster-admin` access on the Azure Local cluster.
- `rad` CLI and `kubectl` installed on the machine running commands (see Stage 0 for connectivity options).
- **This guide is optimized for LocalBox** but the Radius setup steps (Stages 1–5) apply to any Azure Local cluster with AKS enabled by Azure Arc deployed and direct network line-of-sight to the Kubernetes API server (typically 10.x.x.x:6443).

### Important: Kubernetes Connectivity Architecture

**Why not Arc proxy (`az connectedk8s proxy`)?**

Azure Arc's proxy endpoint has limitations for certain custom resource APIs, particularly Radius UCP operations (e.g., `rad group create`, `rad env create`). These operations require direct Kubernetes API access and will fail with an **unsupported API version** error when routed through the Arc proxy.

**Solution:**
- Use **Azure Bastion** (Standard SKU recommended) to RDP/SSH into the management VM where the Kubernetes API is natively reachable.
- Or use direct network routing if your client machine has line-of-sight to the control plane subnet.

**Scope and Applicability**

This guide is **optimized for LocalBox** (the [Azure Arc Jumpstart](https://jumpstart.azure.com/azure_jumpstart_localbox/getting_started) virtualized sandbox: nested Hyper-V, 2-node Azure Local, AKS enabled by Arc, management VM with AD). Stage 0 (connectivity) is LocalBox-specific because it uses Azure Bastion to reach the management VM.

**However, Stages 1–5 (tool installation and Radius setup) are generic and will work on any Azure Local cluster with:**
- AKS enabled by Azure Arc deployed
- Direct network access to the Kubernetes API server (10.x.x.x:6443)
- A machine (or VM) with line-of-sight to that control plane

For non-LocalBox deployments, adapt Stage 0 connectivity guidance to your environment's topology.

## Stage 0 — Connect to the management VM via Azure Bastion

### Option A: Browser-based RDP via Azure Portal (Recommended)

1. **Azure Portal:** Navigate to your Azure Local management VM resource.
2. **Click "Bastion"** (or **"Connect" > "Bastion"** if Bastion SKU is Standard).
3. Set:
   - **Authentication type:** Password (or SSH key if supported)
   - **Username:** `arcdemo` (or your admin user)
   - **Password:** Your VM admin password
4. Click **Open** to launch the browser-based RDP session.

**From the Bastion session**, open **Administrator PowerShell** and proceed to Stage 1.

### Option B: Native RDP client via Azure CLI (if native client support is enabled)

If your Bastion has **native client support** enabled, use `az network bastion rdp` from your local Azure CLI:

```powershell
az network bastion rdp `
  --name <bastion-name> `
  --resource-group <resource-group> `
  --target-resource-id "/subscriptions/<subscription-id>/resourceGroups/<resource-group>/providers/Microsoft.Compute/virtualMachines/<vm-name>"
```

This opens RDP in your native client. Then proceed to Stage 1.

---

## Stage 1 — Install required tools

### 1.1 Install Azure CLI, kubectl, and rad CLI

Run this in **Administrator PowerShell** on the management VM:

```powershell
$oldProgress = $ProgressPreference
$oldVerbose  = $VerbosePreference
try {
  $ProgressPreference = 'SilentlyContinue'
  $VerbosePreference  = 'SilentlyContinue'
  [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

  $tools = "C:\Tools"
  New-Item -ItemType Directory -Force -Path $tools | Out-Null

  function Save-WebFile {
    param([string]$Url, [string]$Destination)
    try {
      Start-BitsTransfer -Source $Url -Destination $Destination -ErrorAction Stop
    } catch {
      Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing
    }
  }

  # Install kubectl
  $stableTxt = Join-Path $env:TEMP "k8s-stable.txt"
  Save-WebFile -Url "https://dl.k8s.io/release/stable.txt" -Destination $stableTxt
  $kver = (Get-Content $stableTxt -Raw).Trim()
  Save-WebFile -Url "https://dl.k8s.io/release/$kver/bin/windows/amd64/kubectl.exe" -Destination (Join-Path $tools "kubectl.exe")

  # Install rad CLI
  $release = Invoke-RestMethod -Uri "https://api.github.com/repos/radius-project/radius/releases/latest"
  $asset = $release.assets | Where-Object { $_.name -match 'rad' -and $_.name -match 'windows' -and $_.name -match 'amd64' -and $_.name -match '\.(zip|exe)$' } | Select-Object -First 1
  if (-not $asset) { throw "Could not find Windows amd64 rad asset." }
  $radDownload = Join-Path $env:TEMP $asset.name
  Save-WebFile -Url $asset.browser_download_url -Destination $radDownload

  if ($radDownload.ToLower().EndsWith(".zip")) {
    $extract = Join-Path $env:TEMP "rad_extract"
    Remove-Item -Path $extract -Recurse -Force -ErrorAction SilentlyContinue
    Expand-Archive -Path $radDownload -DestinationPath $extract -Force
    $radExe = Get-ChildItem -Path $extract -Recurse -Filter "rad.exe" | Select-Object -First 1
    Copy-Item $radExe.FullName (Join-Path $tools "rad.exe") -Force
  } else {
    Copy-Item $radDownload (Join-Path $tools "rad.exe") -Force
  }

  # Add C:\Tools to PATH
  $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
  if (($machinePath -split ';') -notcontains $tools) {
    [Environment]::SetEnvironmentVariable("Path", "$tools;$machinePath", "Machine")
  }
  if (($env:Path -split ';') -notcontains $tools) {
    $env:Path = "$tools;$env:Path"
  }

  # Install Azure CLI
  Invoke-WebRequest -Uri https://aka.ms/installazurecliwindowsx64 -OutFile .\AzureCLI.msi
  Start-Process msiexec.exe -Wait -ArgumentList '/I .\AzureCLI.msi /quiet'
  Remove-Item .\AzureCLI.msi

  # Refresh PATH for current session
  $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

  # Verify installations
  kubectl version --client
  rad version
  az --version
}
finally {
  $ProgressPreference = $oldProgress
  $VerbosePreference  = $oldVerbose
}
```

### 1.2 Retrieve kubeconfig from Azure Local cluster

Use `az aksarc` to retrieve the direct kubeconfig (requires `Azure Kubernetes Service Arc Cluster Admin` role):

```powershell
# Install/update Azure CLI extensions
az config set extension.dynamic_install_allow_preview=true
az config set extension.use_dynamic_install=yes_without_prompt
az extension add --name connectedk8s --upgrade
az extension add --name aksarc --allow-preview true

# Retrieve admin kubeconfig
az aksarc get-credentials \
  --resource-group <your-resource-group> \
  --name <your-cluster-name> \
  --admin \
  --overwrite-existing
```

This kubeconfig is **direct** (server URL = `https://10.x.x.x:6443`) and will work for all Radius commands.

### 1.3 Verify cluster access and direct connectivity

```powershell
# Check kubectl context (should show a direct server URL, not /proxies/)
kubectl config view --minify -o jsonpath='{.clusters[0].cluster.server}'; Write-Host ""

# Verify reachability to control plane
$apiServer = kubectl config view --minify -o jsonpath='{.clusters[0].cluster.server}' -replace 'https://', '' -replace ':6443.*', ''
Test-NetConnection $apiServer -Port 6443

# Confirm cluster nodes
kubectl get nodes
kubectl config current-context
```

Expected output:
- Server URL like `https://10.10.0.105:6443` (not containing `/proxies/`)
- `TcpTestSucceeded : True` (direct control plane reachability)
- Nodes listed with Ready status
- Context matching your cluster name

### 1.4 Install Radius into the Azure Local cluster

With the Azure Local cluster as the current `kubectl` context:

```powershell
rad install kubernetes
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.

If this fails with `response status code 403: denied` while downloading the Radius Helm chart from `ghcr.io`, clear stale GitHub Container Registry credentials and retry:

```powershell
helm registry logout ghcr.io
docker logout ghcr.io
rad install kubernetes
```

### 1.5 Verify all Radius pods are healthy

```powershell
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Expected: All pods in `radius-system` should be `Running` or `Completed`.

## Stage 2 — Configure workspaces and environments

### 2.1 Create workspace for Azure Local environment

For **connected** Azure Local:

```powershell
rad workspace create kubernetes ws-local-prod `
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-prod
```

For **disconnected** Azure Local, use a distinct workspace name:

```powershell
rad workspace create kubernetes ws-local-disconnected-prod `
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-disconnected-prod
```

### 2.2 Create resource groups

```powershell
rad group create rg-trading
rad group switch rg-trading
```

### 2.3 Create environment

For connected Azure Local (with optional Azure backend):

```powershell
rad env create env-local-prod --group rg-trading --kubernetes-namespace prod
rad env switch env-local-prod
```

For disconnected Azure Local (in-cluster recipes only):

```powershell
rad env create env-local-disconnected-prod --group rg-trading --kubernetes-namespace prod-disconnected
rad env switch env-local-disconnected-prod
```

### 2.4 (Optional) Register Azure cloud provider (connected only)

Only if your Azure Local has reliable connectivity to Azure and you want cloud-based resources:

```powershell
$AZURE_SUBSCRIPTION = "<your-subscription-id>"
$RESOURCE_GROUP = "<your-azure-resource-group>"

rad env update env-local-prod `
    --azure-subscription-id "$AZURE_SUBSCRIPTION" `
    --azure-resource-group "$RESOURCE_GROUP"
```

For disconnected scenarios, skip this step entirely.

## Stage 3 — Validate setup

```powershell
rad workspace list
rad env list
rad group list
```

Expected output:
- Workspace active (`ws-local-prod` or `ws-local-disconnected-prod`)
- Environment listed with status `Succeeded`
- `rg-trading` resource group listed

## Stage 4 — Optional: Explore the dashboard

```powershell
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in a browser and verify:
- Environments tab shows your environment
- Resource groups tab shows `rg-trading`
- Applications tab is empty (expected at this stage)
- Cloud provider registration status (if configured)

## Stage 5 — Data synchronization setup (optional, for hybrid scenarios)

If your Azure Local needs to sync data with cloud-based databases or services during connectivity windows, refer to [docs/data-sync/README.md](../../docs/data-sync/README.md) for platform-specific guidance (PostgreSQL, SQL Server, CDC patterns, etc.).

## Troubleshooting

### Error: "unsupported API version" when running rad commands

**Symptom:** `rad group create`, `rad env create`, or similar commands fail with HTTP 400 and "unsupported API version".

**Root cause:** You are using a kubeconfig with a proxy endpoint (containing `/proxies/` in the server URL). Arc proxy does not support Radius UCP resource operations.

**Solution:**
1. Verify your kubeconfig server URL:
   ```powershell
   kubectl config view --minify -o jsonpath='{.clusters[0].cluster.server}'; Write-Host ""
   ```
   - ❌ If it contains `/proxies/`, you are using Arc proxy — **stop here**.
   - ✓ If it shows `https://10.x.x.x:6443`, you are using direct API access — **proceed with rad commands**.

2. If using Arc proxy, retrieve direct kubeconfig from management VM (Stage 1.2):
   ```powershell
   az aksarc get-credentials --resource-group <rg> --name <cluster> --admin --overwrite-existing
   ```

### Radius pods not starting

```powershell
kubectl get pods -n radius-system
kubectl describe pod <pod-name> -n radius-system
kubectl logs <pod-name> -n radius-system
```

Common issues: resource constraints, image pull failures, incorrect cluster configuration.

### kubectl or rad commands not found

Ensure C:\Tools is in PATH and you have restarted PowerShell after installation:

```powershell
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
kubectl version --client
rad version
```

### Cannot connect via Azure Bastion RDP

- Ensure Bastion SKU is **Standard** (supports native RDP/SSH) and that **Native client support** is enabled.
- If using `az network bastion rdp`, verify that the `--target-resource-id` has **correct casing** for the provider namespace: `Microsoft.Compute` (capital M and C), not `microsoft.compute`. The CLI performs case-sensitive matching here, even though Azure Resource Manager itself is case-insensitive. Incorrect casing causes "Unexpected internal error" with no helpful diagnostic.

## Notes

- **Connectivity model (LocalBox):** Use Azure Bastion (Standard SKU with native client support enabled) to RDP into the management VM (**LocalBox-Client** / **AzLMGMT**). Do **not** use `az connectedk8s proxy` for Radius CLI operations — it has limitations for Radius UCP resource APIs. Always use direct kubeconfig retrieved via `az aksarc get-credentials`.
- **Management VM naming:** In LocalBox, the management VM is named **AzLMGMT** in Hyper-V or **LocalBox-Client** in Azure Resource Manager.
- **Bastion connection options:**
  - Browser-based RDP via Azure Portal (Stage 0, Option A)
  - Native RDP client via `az network bastion rdp` if native client support is enabled (Stage 0, Option B)
- **Tools installation:** If MSI installation is blocked by policy on the management VM, use the alternative PowerShell-based download method provided in Stage 1.1, which does not require admin policy approval for MSI execution.
- **Applicability:** Stage 0 (connectivity) is LocalBox-specific. Stages 1\u20135 (tool installation, kubeconfig retrieval, Radius workspace/environment setup) are generic and apply to any Azure Local cluster with AKS deployed and direct API server line-of-sight.

## Next steps

Proceed to Challenge 2 — Recipe authoring and application deployment on Azure Local.

For data synchronization in hybrid scenarios (if cloud connectivity is available), refer to [docs/data-sync/README.md](../../docs/data-sync/README.md) for platform-specific guidance.
