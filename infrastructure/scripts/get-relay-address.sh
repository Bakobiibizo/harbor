#!/bin/bash
# Get the relay address for a deployed Harbor relay stack.
#
# Usage:
#   ./get-relay-address.sh                     # defaults
#   ./get-relay-address.sh --name my-relay     # custom stack name

set -euo pipefail

STACK_NAME="harbor-relay"
REGION="us-east-1"

while [[ $# -gt 0 ]]; do
  case $1 in
    --name)   STACK_NAME="$2"; shift 2 ;;
    --region) REGION="$2"; shift 2 ;;
    --help|-h)
      echo "Usage: $0 [--name STACK_NAME] [--region REGION]"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

RELAY_ADDRESS=$(aws ssm get-parameter \
  --name "/harbor/$STACK_NAME/relay-address" \
  --query 'Parameter.Value' \
  --output text \
  --region "$REGION" 2>/dev/null || true)

if [ -z "$RELAY_ADDRESS" ] || [ "$RELAY_ADDRESS" = "None" ]; then
  echo "No relay address found for stack '$STACK_NAME' in $REGION"
  echo "The relay may still be starting. Try again in a minute."
  exit 1
fi

echo "$RELAY_ADDRESS"
