# Trading frontend

Set `COMPUTE_PLATFORM=k3s` for a local K3s deployment or `aks` for AKS
(the default). Radius supplies this with the `computePlatform` parameter.
The K3s bootstrap command includes `--parameters computePlatform=k3s`.
The authenticated `/config` response exposes the setting to the capability
panel: K3s is yellow and AKS is green in both connectivity scenarios.
Outage simulation and the selected AI provider do not change the compute
platform. Invalid values stop frontend startup explicitly.

For an existing Radius deployment, preserve the complete frontend resource
configuration when updating its `COMPUTE_PLATFORM` environment entry. Update
the retained deployment parameters too so a later Bicep deployment preserves
the setting.

Run frontend tests with `npm test --prefix src/frontend`.
