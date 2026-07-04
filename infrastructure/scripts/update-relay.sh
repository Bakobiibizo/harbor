#!/bin/bash
# Update the relay binary on a running EC2 instance without tearing down the stack.
#
# Prerequisites:
#   1. Build the relay: ./scripts/build-relay.sh
#   2. Push to GitHub: git add relay-server/bin/ && git commit && git push
#   3. Run this script: ./infrastructure/scripts/update-relay.sh
#
# Usage:
#   ./update-relay.sh                          # defaults (community stack: harbor-relay, region: us-east-1)
#   ./update-relay.sh --type relay             # lightweight relay service name
#   ./update-relay.sh --name my-relay          # custom stack name
#   ./update-relay.sh --region us-west-2       # different region

set -euo pipefail

# Defaults
STACK_NAME="harbor-relay"
REGION="us-east-1"
TEMPLATE_TYPE="community"
EXPECTED_SHA256="a4b5f161fa78cb1d5453831a3c0bb28c3281b0db581352989a83eb088bf6e079"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --name)    STACK_NAME="$2"; shift 2 ;;
    --region)  REGION="$2"; shift 2 ;;
    --type)    TEMPLATE_TYPE="$2"; shift 2 ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "  --name NAME      Stack name (default: harbor-relay)"
      echo "  --region REGION  AWS region (default: us-east-1)"
      echo "  --type TYPE      Template/service type: 'community' or 'relay' (default: community)"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

if [ "$TEMPLATE_TYPE" = "community" ]; then
  SERVICE_NAME="${STACK_NAME}-community-relay"
elif [ "$TEMPLATE_TYPE" = "relay" ]; then
  SERVICE_NAME="${STACK_NAME}-relay"
else
  echo "ERROR: unknown --type '$TEMPLATE_TYPE' (expected 'community' or 'relay')"
  exit 1
fi
BINARY_URL="https://github.com/bakobiibizo/harbor/raw/main/relay-server/bin/harbor-relay"

echo "=== Updating Harbor Relay ==="
echo "Stack:   $STACK_NAME"
echo "Region:  $REGION"
echo "Type:    $TEMPLATE_TYPE"
echo "Service: $SERVICE_NAME"
echo ""

# Get instance ID from CloudFormation
echo "[1/5] Finding EC2 instance..."
INSTANCE_ID=$(aws cloudformation describe-stack-resources \
  --stack-name "$STACK_NAME" \
  --region "$REGION" \
  --query "StackResources[?ResourceType=='AWS::EC2::Instance'].PhysicalResourceId" \
  --output text 2>/dev/null)

if [ -z "$INSTANCE_ID" ] || [ "$INSTANCE_ID" = "None" ]; then
  echo "ERROR: No EC2 instance found for stack '$STACK_NAME' in $REGION"
  exit 1
fi
echo "       Instance: $INSTANCE_ID"

# Check instance is running and SSM-managed
echo "[2/5] Checking SSM connectivity..."
SSM_STATUS=$(aws ssm describe-instance-information \
  --filters "Key=InstanceIds,Values=$INSTANCE_ID" \
  --region "$REGION" \
  --query "InstanceInformationList[0].PingStatus" \
  --output text 2>/dev/null || echo "Unavailable")

if [ "$SSM_STATUS" != "Online" ]; then
  echo "ERROR: Instance $INSTANCE_ID is not reachable via SSM (status: $SSM_STATUS)"
  echo "       Make sure the instance is running and SSM agent is active."
  exit 1
fi
echo "       SSM status: Online"

# Send update command via SSM
echo "[3/5] Downloading new binary and restarting service..."
PARAMETERS_FILE=$(mktemp)
trap 'rm -f "$PARAMETERS_FILE"' EXIT
python3 - "$PARAMETERS_FILE" "$SERVICE_NAME" "$BINARY_URL" "$EXPECTED_SHA256" <<'PY'
import json
import sys

path, service_name, binary_url, expected_sha256 = sys.argv[1:]
commands = [
    "echo '[+] Downloading new binary...'",
    f"curl -fSL --retry 3 -o /tmp/harbor-relay-new '{binary_url}'",
    "chmod +x /tmp/harbor-relay-new",
    "echo '[+] Verifying binary sha256...'",
    "ACTUAL_SHA256=$(sha256sum /tmp/harbor-relay-new | awk '{print $1}')",
    f"test \"$ACTUAL_SHA256\" = '{expected_sha256}' || {{ echo sha256 mismatch: expected {expected_sha256} got $ACTUAL_SHA256; rm -f /tmp/harbor-relay-new; exit 1; }}",
    "echo '[+] Stopping relay service...'",
    f"systemctl stop {service_name} || true",
    "echo '[+] Replacing binary...'",
    "install -m 0755 /tmp/harbor-relay-new /usr/local/bin/harbor-relay",
    "rm -f /tmp/harbor-relay-new",
    "echo '[+] Starting relay service...'",
    f"systemctl start {service_name}",
    "sleep 3",
    "echo '[+] Service status:'",
    f"systemctl is-active {service_name}",
    "echo '[+] Update complete.'",
]
with open(path, "w", encoding="utf-8") as fh:
    json.dump({"commands": commands}, fh)
PY

COMMAND_ID=$(aws ssm send-command \
  --instance-ids "$INSTANCE_ID" \
  --region "$REGION" \
  --document-name "AWS-RunShellScript" \
  --comment "Update harbor-relay binary" \
  --timeout-seconds 120 \
  --parameters "file://$PARAMETERS_FILE" \
  --query "Command.CommandId" \
  --output text)

echo "       Command ID: $COMMAND_ID"

# Wait for command to complete
echo "[4/5] Waiting for update to complete..."
aws ssm wait command-executed \
  --command-id "$COMMAND_ID" \
  --instance-id "$INSTANCE_ID" \
  --region "$REGION" 2>/dev/null || true

# Get command output
STATUS=$(aws ssm get-command-invocation \
  --command-id "$COMMAND_ID" \
  --instance-id "$INSTANCE_ID" \
  --region "$REGION" \
  --query "Status" \
  --output text 2>/dev/null)

OUTPUT=$(aws ssm get-command-invocation \
  --command-id "$COMMAND_ID" \
  --instance-id "$INSTANCE_ID" \
  --region "$REGION" \
  --query "StandardOutputContent" \
  --output text 2>/dev/null)

echo ""
echo "--- Remote Output ---"
echo "$OUTPUT"
echo "--- End Output ---"
echo ""

if [ "$STATUS" = "Success" ]; then
  echo "[5/5] Relay updated successfully!"
  echo ""
  # Show relay address
  RELAY_ADDR=$(aws ssm get-parameter \
    --name "/harbor/$STACK_NAME/relay-address" \
    --region "$REGION" \
    --query "Parameter.Value" \
    --output text 2>/dev/null || echo "(not found)")
  echo "Relay address: $RELAY_ADDR"
else
  echo "[5/5] Update FAILED (status: $STATUS)"
  # Show error output
  ERROR_OUTPUT=$(aws ssm get-command-invocation \
    --command-id "$COMMAND_ID" \
    --instance-id "$INSTANCE_ID" \
    --region "$REGION" \
    --query "StandardErrorContent" \
    --output text 2>/dev/null)
  if [ -n "$ERROR_OUTPUT" ]; then
    echo "Error output:"
    echo "$ERROR_OUTPUT"
  fi
  exit 1
fi
