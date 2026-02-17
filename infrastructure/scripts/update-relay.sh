#!/bin/bash
# Update the relay binary on a running EC2 instance without tearing down the stack.
#
# Prerequisites:
#   1. Build the relay: ./scripts/build-relay.sh
#   2. Push to GitHub: git add relay-server/bin/ && git commit && git push
#   3. Run this script: ./infrastructure/scripts/update-relay.sh
#
# Usage:
#   ./update-relay.sh                     # defaults (stack: harbor-relay, region: us-east-1)
#   ./update-relay.sh --name my-relay     # custom stack name
#   ./update-relay.sh --region us-west-2  # different region

set -euo pipefail

# Defaults
STACK_NAME="harbor-relay"
REGION="us-east-1"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --name)    STACK_NAME="$2"; shift 2 ;;
    --region)  REGION="$2"; shift 2 ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "  --name NAME      Stack name (default: harbor-relay)"
      echo "  --region REGION   AWS region (default: us-east-1)"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

SERVICE_NAME="${STACK_NAME}-relay"
BINARY_URL="https://github.com/bakobiibizo/harbor/raw/main/relay-server/bin/harbor-relay"

echo "=== Updating Harbor Relay ==="
echo "Stack:   $STACK_NAME"
echo "Region:  $REGION"
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
COMMAND_ID=$(aws ssm send-command \
  --instance-ids "$INSTANCE_ID" \
  --region "$REGION" \
  --document-name "AWS-RunShellScript" \
  --comment "Update harbor-relay binary" \
  --timeout-seconds 120 \
  --parameters commands="[
    \"echo '[+] Stopping relay service...'\",
    \"systemctl stop $SERVICE_NAME || true\",
    \"echo '[+] Downloading new binary...'\",
    \"curl -fSL --retry 3 -o /tmp/harbor-relay-new '$BINARY_URL'\",
    \"chmod +x /tmp/harbor-relay-new\",
    \"echo '[+] Replacing binary...'\",
    \"mv /tmp/harbor-relay-new /usr/local/bin/harbor-relay\",
    \"echo '[+] Starting relay service...'\",
    \"systemctl start $SERVICE_NAME\",
    \"sleep 3\",
    \"echo '[+] Service status:'\",
    \"systemctl is-active $SERVICE_NAME\",
    \"echo '[+] Update complete.'\"
  ]" \
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
