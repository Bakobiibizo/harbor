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
#   ./update-relay.sh --artifact-url URL --sha256 SHA256

set -euo pipefail

# Defaults
STACK_NAME="harbor-relay"
REGION="us-east-1"
TEMPLATE_TYPE="community"
EXPECTED_SHA256="b6d3a64b27c818ca67b1d9cccbb8a0629da641b5d10438e93001f751221eba40"
BINARY_URL="https://github.com/bakobiibizo/harbor/raw/main/relay-server/bin/harbor-relay"
IDENTITY_NAMESPACE="harbor.social"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --name)    STACK_NAME="$2"; shift 2 ;;
    --region)  REGION="$2"; shift 2 ;;
    --type)    TEMPLATE_TYPE="$2"; shift 2 ;;
    --artifact-url) BINARY_URL="$2"; shift 2 ;;
    --sha256) EXPECTED_SHA256="$2"; shift 2 ;;
    --identity-namespace) IDENTITY_NAMESPACE="$2"; shift 2 ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "  --name NAME      Stack name (default: harbor-relay)"
      echo "  --region REGION  AWS region (default: us-east-1)"
      echo "  --type TYPE      Template/service type: 'community' or 'relay' (default: community)"
      echo "  --artifact-url URL  Immutable relay artifact URL"
      echo "  --sha256 SHA256     Expected artifact checksum"
      echo "  --identity-namespace HOST  Relay name authority (default: harbor.social)"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

if [[ ! "$EXPECTED_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "ERROR: --sha256 must be a lowercase 64-character SHA-256 digest" >&2
  exit 1
fi
if [[ ! "$IDENTITY_NAMESPACE" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])$ ]]; then
  echo "ERROR: --identity-namespace must be a canonical lowercase DNS hostname" >&2
  exit 1
fi
if [[ ! "$BINARY_URL" =~ ^https:// ]]; then
  echo "ERROR: --artifact-url must use HTTPS" >&2
  exit 1
fi

if [ "$TEMPLATE_TYPE" = "community" ]; then
  SERVICE_NAME="${STACK_NAME}-community-relay"
elif [ "$TEMPLATE_TYPE" = "relay" ]; then
  SERVICE_NAME="${STACK_NAME}-relay"
else
  echo "ERROR: unknown --type '$TEMPLATE_TYPE' (expected 'community' or 'relay')"
  exit 1
fi
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
python3 - "$PARAMETERS_FILE" "$SERVICE_NAME" "$BINARY_URL" "$EXPECTED_SHA256" "$IDENTITY_NAMESPACE" <<'PY'
import json
import sys

path, service_name, binary_url, expected_sha256, identity_namespace = sys.argv[1:]
commands = [
    "set -euo pipefail",
    "echo '[+] Downloading new binary...'",
    f"curl -fSL --retry 3 -o /tmp/harbor-relay-new '{binary_url}'",
    "chmod +x /tmp/harbor-relay-new",
    "echo '[+] Verifying binary sha256...'",
    "ACTUAL_SHA256=$(sha256sum /tmp/harbor-relay-new | awk '{print $1}')",
    f"test \"$ACTUAL_SHA256\" = '{expected_sha256}' || {{ echo sha256 mismatch: expected {expected_sha256} got $ACTUAL_SHA256; rm -f /tmp/harbor-relay-new; exit 1; }}",
    "/tmp/harbor-relay-new --version",
    "/tmp/harbor-relay-new --help | grep -q -- '--identity-namespace'",
    "BACKUP_SUFFIX=$(date -u +%Y%m%dT%H%M%SZ)",
    "cp -a /usr/local/bin/harbor-relay /usr/local/bin/harbor-relay.rollback-$BACKUP_SUFFIX",
    f"cp -a /etc/systemd/system/{service_name}.service /etc/systemd/system/{service_name}.service.rollback-$BACKUP_SUFFIX",
    "if [ -d /var/lib/harbor-relay/data ]; then cp -a /var/lib/harbor-relay/data /var/lib/harbor-relay/data.rollback-$BACKUP_SUFFIX; fi",
    "echo '[+] Stopping relay service...'",
    f"systemctl stop {service_name}",
    "echo '[+] Replacing binary...'",
    "install -m 0755 /tmp/harbor-relay-new /usr/local/bin/harbor-relay",
    "rm -f /tmp/harbor-relay-new",
    f"sed -i '/^ExecStart=/ {{ /--identity-namespace/! s/$/ --identity-namespace {identity_namespace}/; }}' /etc/systemd/system/{service_name}.service",
    "systemctl daemon-reload",
    "echo '[+] Starting relay service...'",
    f"if ! systemctl start {service_name}; then cp -a /usr/local/bin/harbor-relay.rollback-$BACKUP_SUFFIX /usr/local/bin/harbor-relay; cp -a /etc/systemd/system/{service_name}.service.rollback-$BACKUP_SUFFIX /etc/systemd/system/{service_name}.service; systemctl daemon-reload; systemctl start {service_name}; echo '[!] Update failed and relay binary/service were rolled back'; exit 1; fi",
    "sleep 3",
    "echo '[+] Service status:'",
    f"systemctl is-active {service_name}",
    "/usr/local/bin/harbor-relay --version",
    f"systemctl show {service_name} --property=ExecStart --no-pager",
    "sha256sum /usr/local/bin/harbor-relay",
    "echo \"[+] Rollback suffix: $BACKUP_SUFFIX\"",
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
