# Harbor Infrastructure - Relay Server Templates

This directory contains AWS CloudFormation templates for deploying relay servers that support Harbor's P2P networking.

## Two Templates

| Template | File | Use Case |
|----------|------|----------|
| **Lightweight Relay** | `relay-cloudformation.yaml` | NAT traversal only — minimal resource usage |
| **Community Relay** | `community-relay-cloudformation.yaml` | NAT traversal + community boards with SQLite storage |

Both templates:
- Generate a **unique identity** per deployment (no two relays share a peer ID)
- Write the relay address to **SSM Parameter Store** automatically
- Run as a systemd service with automatic restarts
- Use an **Elastic IP** for a stable address

### Service Naming

The systemd service name is based on your CloudFormation stack name:

| Template | Stack Name | Service Name |
|----------|------------|--------------|
| Lightweight | `my-relay` | `my-relay-relay` |
| Community | `my-community` | `my-community-community-relay` |

## Prerequisites

1. **AWS Account** — Sign up at https://aws.amazon.com/free/
   - Free tier includes 750 hours/month of t2.micro for the first 12 months
   - That's enough to run one relay server 24/7 for free!

2. **(Optional) EC2 Key Pair** — For SSH access to the server
   - Create at: AWS Console > EC2 > Key Pairs > Create key pair

## One-Click Deploy

### Lightweight Relay

| Region | Deploy |
|--------|--------|
| US East (N. Virginia) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/relay-cloudformation.yaml) |
| US West (Oregon) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-west-2#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/relay-cloudformation.yaml) |
| EU (Ireland) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=eu-west-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/relay-cloudformation.yaml) |
| Asia Pacific (Tokyo) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=ap-northeast-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/relay-cloudformation.yaml) |

### Community Relay

| Region | Deploy |
|--------|--------|
| US East (N. Virginia) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/new?stackName=harbor-community&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |
| US West (Oregon) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-west-2#/stacks/new?stackName=harbor-community&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |
| EU (Ireland) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=eu-west-1#/stacks/new?stackName=harbor-community&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |
| Asia Pacific (Tokyo) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=ap-northeast-1#/stacks/new?stackName=harbor-community&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |

## Manual Deployment via AWS CLI

```bash
# Deploy a lightweight relay
aws cloudformation create-stack \
  --stack-name harbor-relay \
  --template-body file://relay-cloudformation.yaml \
  --capabilities CAPABILITY_NAMED_IAM

# Deploy a community relay
aws cloudformation create-stack \
  --stack-name harbor-community \
  --template-body file://community-relay-cloudformation.yaml \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameters \
    ParameterKey=CommunityName,ParameterValue="My Community"

# Check deployment status
aws cloudformation describe-stacks --stack-name harbor-relay

# Get outputs
aws cloudformation describe-stacks --stack-name harbor-relay \
  --query 'Stacks[0].Outputs'
```

## Getting Your Relay Address

Each relay generates a unique identity on first start. The full relay address (IP + port + peer ID) is written to SSM Parameter Store automatically.

**Option 1: SSM Parameter Store (easiest, no SSH needed)**

```bash
# Replace harbor-relay with your stack name
aws ssm get-parameter \
  --name "/harbor/harbor-relay/relay-address" \
  --query 'Parameter.Value' --output text
```

Or check the stack's **GetRelayAddress** output for the exact command.

**Option 2: AWS Console**

Check the stack's **SSMConsoleLink** output for a direct link to the parameter.

**Option 3: SSH (if you provided a key pair)**

```bash
ssh -i your-key.pem ec2-user@<public-ip>
# Replace SERVICE_NAME with the value from the ServiceName stack output
journalctl -u SERVICE_NAME --no-pager | grep "Local Peer ID"
```

## Adding Your Relay to Harbor

Once you have the relay address from SSM, paste it into:

**Harbor > Network > Advanced > Add Relay Address**

The address format is:
```
/ip4/<PUBLIC_IP>/tcp/4001/p2p/<PEER_ID>
```

## Cost Breakdown

| Resource | Free Tier | After Free Tier |
|----------|-----------|-----------------|
| EC2 t2.micro | 750 hours/month (1 year) | ~$8.50/month |
| EBS Storage (8GB gp3) | 30GB free | ~$0.64/month |
| Data Transfer | 100GB out free | $0.09/GB |
| Elastic IP | Free when attached | $3.60/month if unused |

**Total estimated cost:**
- First 12 months: **$0** (free tier)
- After free tier: **~$10-14/month**

## Monitoring

```bash
# SSH or SSM into the server, then:

# View logs (replace SERVICE_NAME with your actual service name)
journalctl -u SERVICE_NAME -f

# Check relay status
systemctl status SERVICE_NAME
```

## Troubleshooting

**Relay not starting:**
```bash
# Check service status
systemctl status SERVICE_NAME

# View detailed logs
journalctl -u SERVICE_NAME -n 100

# Check the setup log for download or startup errors
cat /var/log/user-data.log
```

**Can't connect from Harbor:**
1. Verify security group allows inbound on port 4001 (TCP and UDP)
2. Check that the relay address in SSM is correct and complete
3. Ensure the relay service is running: `systemctl status SERVICE_NAME`

**High memory usage:**
Reduce `MaxReservations` and `MaxCircuits` parameters in the CloudFormation stack.

## Cleanup

To delete all resources:
```bash
aws cloudformation delete-stack --stack-name harbor-relay
```

This will remove the EC2 instance, VPC, security groups, and all associated resources.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     AWS Cloud                            │
│  ┌───────────────────────────────────────────────────┐  │
│  │                 VPC (10.0.0.0/16)                 │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │           Public Subnet (10.0.1.0/24)       │  │  │
│  │  │  ┌───────────────────────────────────────┐  │  │  │
│  │  │  │         EC2 Instance (t2.micro)       │  │  │  │
│  │  │  │  ┌─────────────────────────────────┐  │  │  │  │
│  │  │  │  │   harbor-relay (systemd service) │  │  │  │  │
│  │  │  │  │   Port 4001 (TCP/UDP)           │  │  │  │  │
│  │  │  │  └─────────────────────────────────┘  │  │  │  │
│  │  │  └───────────────────────────────────────┘  │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
│                          │                               │
│                    Elastic IP                            │
│                          │                               │
└──────────────────────────┼───────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              │                         │
      ┌───────┴───────┐         ┌───────┴───────┐
      │  Harbor User  │         │  Harbor User  │
      │  (behind NAT) │◄───────►│  (behind NAT) │
      └───────────────┘  relay  └───────────────┘
```

When two Harbor users are both behind NAT:
1. Both connect to the relay server
2. Messages are forwarded through the relay
3. DCUtR (hole punching) attempts to establish a direct connection
4. If successful, subsequent messages go directly peer-to-peer
