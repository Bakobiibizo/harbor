#!/bin/bash
# Allocate and attach an Elastic IP to the current EC2 instance.
# Run this ON the relay instance (via SSH or SSM Session Manager).
#
# Usage: ./attach-eip.sh [region]
#   region defaults to us-east-1

set -euo pipefail

REGION="${1:-us-east-1}"

echo "=== Attaching Elastic IP ==="

# Get instance ID from IMDS
IMDS_TOKEN=$(curl -sX PUT "http://169.254.169.254/latest/api/token" -H "X-aws-ec2-metadata-token-ttl-seconds: 21600")
INSTANCE_ID=$(curl -s -H "X-aws-ec2-metadata-token: $IMDS_TOKEN" http://169.254.169.254/latest/meta-data/instance-id)
echo "Instance: $INSTANCE_ID"

# Check for existing unassociated EIPs tagged for harbor
EXISTING_ALLOC=$(aws ec2 describe-addresses \
  --filters "Name=tag:Application,Values=harbor-chat" \
  --query 'Addresses[?AssociationId==null].AllocationId | [0]' \
  --output text \
  --region "$REGION" 2>/dev/null || true)

if [ -n "$EXISTING_ALLOC" ] && [ "$EXISTING_ALLOC" != "None" ]; then
  echo "Found unassociated harbor EIP: $EXISTING_ALLOC"
  ALLOC_ID="$EXISTING_ALLOC"
else
  echo "Allocating new Elastic IP..."
  ALLOC_ID=$(aws ec2 allocate-address \
    --domain vpc \
    --tag-specifications 'ResourceType=elastic-ip,Tags=[{Key=Name,Value=harbor-relay-eip},{Key=Application,Value=harbor-chat}]' \
    --query 'AllocationId' \
    --output text \
    --region "$REGION")
  echo "Allocated: $ALLOC_ID"
fi

# Associate with this instance
echo "Associating EIP with instance $INSTANCE_ID..."
aws ec2 associate-address \
  --instance-id "$INSTANCE_ID" \
  --allocation-id "$ALLOC_ID" \
  --allow-reassociation \
  --region "$REGION"

# Get the public IP
PUBLIC_IP=$(aws ec2 describe-addresses \
  --allocation-ids "$ALLOC_ID" \
  --query 'Addresses[0].PublicIp' \
  --output text \
  --region "$REGION")

echo ""
echo "=== Done ==="
echo "Elastic IP: $PUBLIC_IP"
echo "Allocation: $ALLOC_ID"
echo ""
echo "Update your relay address to use this IP."
