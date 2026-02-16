#!/bin/bash
# Deploy a Harbor relay CloudFormation stack.
# Run this from your local machine (needs AWS CLI configured).
#
# Usage:
#   ./deploy-stack.sh                          # community relay with defaults
#   ./deploy-stack.sh --type relay             # lightweight relay
#   ./deploy-stack.sh --name my-relay          # custom stack name
#   ./deploy-stack.sh --region us-west-2       # different region
#   ./deploy-stack.sh --community "My Group"   # custom community name

set -euo pipefail

# Defaults
STACK_NAME="harbor-relay"
REGION="us-east-1"
TEMPLATE_TYPE="community"
COMMUNITY_NAME="Harbor Community"
INSTANCE_TYPE="t2.micro"
RELAY_PORT="4001"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --name)       STACK_NAME="$2"; shift 2 ;;
    --region)     REGION="$2"; shift 2 ;;
    --type)       TEMPLATE_TYPE="$2"; shift 2 ;;
    --community)  COMMUNITY_NAME="$2"; shift 2 ;;
    --instance)   INSTANCE_TYPE="$2"; shift 2 ;;
    --port)       RELAY_PORT="$2"; shift 2 ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "  --name NAME        Stack name (default: harbor-relay)"
      echo "  --region REGION    AWS region (default: us-east-1)"
      echo "  --type TYPE        Template: 'community' or 'relay' (default: community)"
      echo "  --community NAME   Community name (default: Harbor Community)"
      echo "  --instance TYPE    EC2 instance type (default: t2.micro)"
      echo "  --port PORT        Relay port (default: 4001)"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# Select template
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ "$TEMPLATE_TYPE" = "community" ]; then
  TEMPLATE_FILE="$SCRIPT_DIR/../community-relay-cloudformation.yaml"
else
  TEMPLATE_FILE="$SCRIPT_DIR/../relay-cloudformation.yaml"
fi

if [ ! -f "$TEMPLATE_FILE" ]; then
  echo "ERROR: Template not found: $TEMPLATE_FILE"
  exit 1
fi

echo "=== Deploying Harbor Relay ==="
echo "Stack:     $STACK_NAME"
echo "Region:    $REGION"
echo "Template:  $TEMPLATE_TYPE"
echo "Instance:  $INSTANCE_TYPE"
echo "Port:      $RELAY_PORT"
if [ "$TEMPLATE_TYPE" = "community" ]; then
  echo "Community: $COMMUNITY_NAME"
fi
echo ""

# Build parameters
PARAMS="ParameterKey=InstanceType,ParameterValue=$INSTANCE_TYPE"
PARAMS="$PARAMS ParameterKey=RelayPort,ParameterValue=$RELAY_PORT"
if [ "$TEMPLATE_TYPE" = "community" ]; then
  PARAMS="$PARAMS ParameterKey=CommunityName,ParameterValue=$COMMUNITY_NAME"
fi

# Check if stack already exists
STACK_STATUS=$(aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --query 'Stacks[0].StackStatus' \
  --output text \
  --region "$REGION" 2>/dev/null || echo "DOES_NOT_EXIST")

if [ "$STACK_STATUS" = "DOES_NOT_EXIST" ]; then
  echo "Creating new stack..."
  aws cloudformation create-stack \
    --stack-name "$STACK_NAME" \
    --template-body "file://$TEMPLATE_FILE" \
    --capabilities CAPABILITY_NAMED_IAM \
    --parameters $PARAMS \
    --region "$REGION"

  echo "Waiting for stack creation..."
  aws cloudformation wait stack-create-complete \
    --stack-name "$STACK_NAME" \
    --region "$REGION"
else
  echo "Stack exists (status: $STACK_STATUS). Updating..."
  aws cloudformation update-stack \
    --stack-name "$STACK_NAME" \
    --template-body "file://$TEMPLATE_FILE" \
    --capabilities CAPABILITY_NAMED_IAM \
    --parameters $PARAMS \
    --region "$REGION" 2>/dev/null || {
      echo "No updates needed (or update failed). Current status: $STACK_STATUS"
      exit 0
    }

  echo "Waiting for stack update..."
  aws cloudformation wait stack-update-complete \
    --stack-name "$STACK_NAME" \
    --region "$REGION"
fi

echo ""
echo "=== Stack Ready ==="

# Show outputs
aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --query 'Stacks[0].Outputs[].[OutputKey,OutputValue]' \
  --output table \
  --region "$REGION"

echo ""
echo "Waiting ~2 minutes for relay to start, then get your address:"
echo "  aws ssm get-parameter --name '/harbor/$STACK_NAME/relay-address' --query 'Parameter.Value' --output text --region $REGION"
