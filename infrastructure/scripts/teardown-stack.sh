#!/bin/bash
# Tear down a Harbor relay CloudFormation stack.
# The identity key and EIP allocation ID in SSM are preserved
# so the next deploy with the same stack name reuses them.
#
# Usage:
#   ./teardown-stack.sh                        # defaults
#   ./teardown-stack.sh --name my-relay        # custom stack name
#   ./teardown-stack.sh --clean                # also remove SSM params (full wipe)

set -euo pipefail

# Defaults
STACK_NAME="harbor-relay"
REGION="us-east-1"
CLEAN=false

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --name)    STACK_NAME="$2"; shift 2 ;;
    --region)  REGION="$2"; shift 2 ;;
    --clean)   CLEAN=true; shift ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "  --name NAME      Stack name (default: harbor-relay)"
      echo "  --region REGION   AWS region (default: us-east-1)"
      echo "  --clean           Also delete SSM params and release EIP (full wipe)"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

echo "=== Tearing Down Harbor Relay ==="
echo "Stack:  $STACK_NAME"
echo "Region: $REGION"
echo "Clean:  $CLEAN"
echo ""

# Check stack exists
STACK_STATUS=$(aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --query 'Stacks[0].StackStatus' \
  --output text \
  --region "$REGION" 2>/dev/null || echo "DOES_NOT_EXIST")

if [ "$STACK_STATUS" = "DOES_NOT_EXIST" ]; then
  echo "Stack $STACK_NAME does not exist."
else
  echo "Stack status: $STACK_STATUS"
  echo "Deleting stack..."

  aws cloudformation delete-stack \
    --stack-name "$STACK_NAME" \
    --region "$REGION"

  echo "Waiting for stack deletion..."
  aws cloudformation wait stack-delete-complete \
    --stack-name "$STACK_NAME" \
    --region "$REGION"

  echo "Stack deleted."
fi

if [ "$CLEAN" = true ]; then
  echo ""
  echo "Cleaning up SSM parameters and EIP..."

  # Release EIP if one was saved
  EIP_ALLOC_ID=$(aws ssm get-parameter \
    --name "/harbor/$STACK_NAME/eip-allocation-id" \
    --query 'Parameter.Value' \
    --output text \
    --region "$REGION" 2>/dev/null || true)

  if [ -n "$EIP_ALLOC_ID" ] && [ "$EIP_ALLOC_ID" != "None" ]; then
    echo "Releasing EIP: $EIP_ALLOC_ID"
    aws ec2 release-address \
      --allocation-id "$EIP_ALLOC_ID" \
      --region "$REGION" 2>/dev/null || echo "  (already released or not found)"
  fi

  # Delete SSM parameters
  for param in "relay-address" "identity-key" "eip-allocation-id"; do
    PARAM_NAME="/harbor/$STACK_NAME/$param"
    echo "Deleting SSM param: $PARAM_NAME"
    aws ssm delete-parameter \
      --name "$PARAM_NAME" \
      --region "$REGION" 2>/dev/null || echo "  (not found)"
  done

  echo ""
  echo "=== Full cleanup complete ==="
  echo "All resources removed. Next deploy will get a new IP and peer ID."
else
  echo ""
  echo "=== Teardown complete ==="
  echo "SSM parameters preserved (identity key, EIP). Redeploy with:"
  echo "  ./deploy-stack.sh --name $STACK_NAME --region $REGION"
  echo ""
  echo "To fully wipe (new IP + peer ID), rerun with --clean"
fi
