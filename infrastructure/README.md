# Harbor Infrastructure - Community Relay Server

This directory contains AWS CloudFormation templates for deploying infrastructure to support Harbor's P2P networking.

## Community Relay Server

The relay server enables NAT traversal for Harbor users and hosts community boards. When users are behind NAT (most home networks), they can connect through this relay to communicate with peers anywhere on the internet.

### Prerequisites

1. **AWS Account** - Sign up at https://aws.amazon.com/free/
   - Free tier includes 750 hours/month of t2.micro for the first 12 months
   - That's enough to run one relay server 24/7 for free!

2. **(Optional) EC2 Key Pair** - For SSH access to the server
   - Create at: AWS Console -> EC2 -> Key Pairs -> Create key pair

### How It Works

The CloudFormation template provisions an EC2 instance with a UserData script that:

1. Downloads a pre-compiled relay binary from the Harbor GitHub repository
2. Verifies the binary's SHA256 checksum for integrity
3. Installs the binary to `/usr/local/bin/harbor-relay`
4. Creates and starts a systemd service (named after the CloudFormation stack, default `harbor-relay`)
5. Waits for the Elastic IP association to stabilize
6. Restarts the service with the correct `--announce-ip` flag
7. Extracts the relay's peer ID and writes the full relay address to SSM Parameter Store

The setup takes approximately 2 minutes. After that, the relay runs as a native systemd service with automatic restarts.

### One-Click Deploy

Click the button for your preferred region:

| Region | Deploy |
|--------|--------|
| US East (N. Virginia) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |
| US West (Oregon) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-west-2#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |
| EU (Ireland) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=eu-west-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |
| Asia Pacific (Tokyo) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=ap-northeast-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/community-relay-cloudformation.yaml) |

### Manual Deployment via AWS CLI

```bash
# Deploy with default settings (t2.micro, port 4001)
aws cloudformation create-stack \
  --stack-name harbor-relay \
  --template-body file://community-relay-cloudformation.yaml \
  --capabilities CAPABILITY_NAMED_IAM

# Deploy with custom settings
aws cloudformation create-stack \
  --stack-name harbor-relay \
  --template-body file://community-relay-cloudformation.yaml \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameters \
    ParameterKey=InstanceType,ParameterValue=t2.micro \
    ParameterKey=RelayPort,ParameterValue=4001 \
    ParameterKey=MaxReservations,ParameterValue=128 \
    ParameterKey=CommunityName,ParameterValue="My Community" \
    ParameterKey=RateLimitMaxRequests,ParameterValue=60 \
    ParameterKey=RateLimitWindowSecs,ParameterValue=60

# Check deployment status
aws cloudformation describe-stacks --stack-name harbor-relay

# Get outputs (including public IP)
aws cloudformation describe-stacks --stack-name harbor-relay \
  --query 'Stacks[0].Outputs'
```

### Getting the Relay's Peer ID

After deployment, you need to get the relay's peer ID to use it in Harbor:

**Option 1: SSM Parameter Store (easiest, no SSH needed)**

The CloudFormation stack automatically writes the full relay address (including peer ID) to SSM Parameter Store. Check the stack outputs for a direct link, or run:
```bash
aws ssm get-parameter --name "/harbor/harbor-relay/relay-address" --query 'Parameter.Value' --output text
```

Note: If you used a custom stack name, replace `harbor-relay` with your stack name in the parameter path.

**Option 2: AWS Systems Manager Session (no SSH needed)**
```bash
# Connect to the instance
aws ssm start-session --target <instance-id>

# Then run:
journalctl -u harbor-relay --no-pager | grep "Peer ID"
```

**Option 3: SSH (if you provided a key pair)**
```bash
ssh -i your-key.pem ec2-user@<public-ip>
journalctl -u harbor-relay --no-pager | grep "Peer ID"
```

The output will look like:
```
Local Peer ID: 12D3KooWAbCdEfGhIjKlMnOpQrStUvWxYz...
```

### Adding Your Relay to Harbor

Once you have the peer ID, the full relay address is:
```
/ip4/<PUBLIC_IP>/tcp/4001/p2p/<PEER_ID>
```

To add this to Harbor's default relays, edit `src-tauri/src/p2p/network.rs`:

```rust
const PUBLIC_RELAYS: &[&str] = &[
    // Your custom relay (add this first for priority)
    "/ip4/YOUR_IP/tcp/4001/p2p/YOUR_PEER_ID",
    // IPFS bootstrap relays (fallback)
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    // ... other relays
];
```

### Cost Breakdown

| Resource | Free Tier | After Free Tier |
|----------|-----------|-----------------|
| EC2 t2.micro | 750 hours/month (1 year) | ~$8.50/month |
| EBS Storage (20GB gp3) | 30GB free | ~$1.60/month |
| Data Transfer | 100GB out free | $0.09/GB |
| Elastic IP | Free when attached | $3.60/month if unused |

**Total estimated cost:**
- First 12 months: **$0** (free tier)
- After free tier: **~$10-14/month**

### Monitoring

**View logs:**
```bash
# Via SSM or SSH into the server, then:
journalctl -u harbor-relay -f
```

**Check relay status:**
```bash
systemctl status harbor-relay
```

Note: The systemd service name matches the CloudFormation stack name. If you used a custom stack name (e.g., `my-relay`), replace `harbor-relay` with that name in the commands above.

### Troubleshooting

**Relay not starting:**
```bash
# Check relay service status
systemctl status harbor-relay

# View detailed logs
journalctl -u harbor-relay -n 100

# Check the setup log for download or startup errors
cat /var/log/user-data.log
```

**Can't connect from Harbor:**
1. Verify security group allows inbound on port 4001 (TCP and UDP)
2. Check that the peer ID in your multiaddress is correct
3. Ensure the relay service is running: `systemctl status harbor-relay`

**High memory usage:**
Reduce `MaxReservations` and `MaxCircuits` parameters in the CloudFormation stack.

### Cleanup

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
