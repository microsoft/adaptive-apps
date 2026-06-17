#!/usr/bin/env bash
# app-wi-setup.sh
#
# Creates a user-assigned managed identity for an application workload and
# federates it with a Kubernetes service account on AKS.
#
# Call once per workload identity needed. For example:
#   ./app-wi-setup.sh backend   $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default
#   ./app-wi-setup.sh frontend  $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default
#
# Usage:
#   $0 <IDENTITY_NAME> <RESOURCE_GROUP> <SUBSCRIPTION_ID> <OIDC_ISSUER_URL> <K8S_NAMESPACE> <K8S_SERVICE_ACCOUNT>
#
# Outputs the client ID of the created identity — pass this to rad deploy as
# --parameters <workload>ClientId=<value>

if [ "$#" -ne 6 ]; then
    echo "Usage: $0 <IDENTITY_NAME> <RESOURCE_GROUP> <SUBSCRIPTION_ID> <OIDC_ISSUER_URL> <K8S_NAMESPACE> <K8S_SERVICE_ACCOUNT>"
    exit 1
fi

IDENTITY_NAME=$1
RESOURCE_GROUP=$2
SUBSCRIPTION_ID=$3
OIDC_ISSUER=$4
K8S_NAMESPACE=$5
K8S_SERVICE_ACCOUNT=$6

# Create the user-assigned managed identity
az identity create \
  --name "$IDENTITY_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --subscription "$SUBSCRIPTION_ID"

CLIENT_ID=$(az identity show \
  --name "$IDENTITY_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --subscription "$SUBSCRIPTION_ID" \
  --query clientId -o tsv)

PRINCIPAL_ID=$(az identity show \
  --name "$IDENTITY_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --subscription "$SUBSCRIPTION_ID" \
  --query principalId -o tsv)

# Create the federated identity credential binding this identity to the k8s service account
az identity federated-credential create \
  --name "aks-wi" \
  --identity-name "$IDENTITY_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --issuer "$OIDC_ISSUER" \
  --subject "system:serviceaccount:${K8S_NAMESPACE}:${K8S_SERVICE_ACCOUNT}" \
  --audiences "api://AzureADTokenExchange"

echo ""
echo "Managed identity created:"
echo "  Name:        $IDENTITY_NAME"
echo "  Client ID:   $CLIENT_ID"
echo "  Principal ID: $PRINCIPAL_ID"
echo ""
echo "Use in rad deploy:"
echo "  --parameters ${IDENTITY_NAME}ClientId=${CLIENT_ID}"
