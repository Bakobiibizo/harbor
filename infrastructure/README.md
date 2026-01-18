# Harbor Infrastructure - libp2p Relay Server

This directory contains AWS CloudFormation templates for deploying infrastructure to support Harbor's P2P networking.

## libp2p Relay Server

The relay server enables NAT traversal for Harbor users. When users are behind NAT (most home networks), they can connect through this relay to communicate with peers anywhere on the internet.

### Prerequisites

1. **AWS Account** - Sign up at https://aws.amazon.com/free/
   - Free tier includes 750 hours/month of t2.micro for the first 12 months
   - That's enough to run one relay server 24/7 for free!

2. **(Optional) EC2 Key Pair** - For SSH access to the server
   - Create at: AWS Console → EC2 → Key Pairs → Create key pair

### One-Click Deploy

Click the button for your preferred region:

| Region | Deploy |
|--------|--------|
| US East (N. Virginia) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/libp2p-relay-cloudformation.yaml) |
| US West (Oregon) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=us-west-2#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/libp2p-relay-cloudformation.yaml) |
| EU (Ireland) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=eu-west-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/libp2p-relay-cloudformation.yaml) |
| Asia Pacific (Tokyo) | [![Launch Stack](https://cdn.rawgit.com/buildkite/cloudformation-launch-stack-button-svg/master/launch-stack.svg)](https://console.aws.amazon.com/cloudformation/home?region=ap-northeast-1#/stacks/new?stackName=harbor-relay&templateURL=https://raw.githubusercontent.com/bakobiibizo/harbor/main/infrastructure/libp2p-relay-cloudformation.yaml) |

### Manual Deployment via AWS CLI

```bash
# Deploy with default settings (t2.micro, port 4001)
aws cloudformation create-stack \
  --stack-name harbor-relay \
  --template-body file://libp2p-relay-cloudformation.yaml \
  --capabilities CAPABILITY_NAMED_IAM

# Deploy with custom settings
aws cloudformation create-stack \
  --stack-name harbor-relay \
  --template-body file://libp2p-relay-cloudformation.yaml \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameters \
    ParameterKey=InstanceType,ParameterValue=t2.micro \
    ParameterKey=RelayPort,ParameterValue=4001 \
    ParameterKey=MaxReservations,ParameterValue=128

# Check deployment status
aws cloudformation describe-stacks --stack-name harbor-relay

# Get outputs (including public IP)
aws cloudformation describe-stacks --stack-name harbor-relay \
  --query 'Stacks[0].Outputs'
```

### Getting the Relay's Peer ID

After deployment, you need to get the relay's peer ID to use it in Harbor:

**Option 1: AWS Systems Manager (no SSH needed)**
```bash
# Connect to the instance
aws ssm start-session --target <instance-id>

# Then run:
docker logs harbor-relay 2>&1 | grep "Peer ID"
```

**Option 2: SSH (if you provided a key pair)**
```bash
ssh -i your-key.pem ec2-user@<public-ip>
docker logs harbor-relay 2>&1 | grep "Peer ID"
```

The output will look like:
```
Peer ID: 12D3KooWAbCdEfGhIjKlMnOpQrStUvWxYz...
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
| EBS Storage (8GB gp3) | 30GB free | ~$0.64/month |
| Data Transfer | 100GB out free | $0.09/GB |
| Elastic IP | Free when attached | $3.60/month if unused |

**Total estimated cost:**
- First 12 months: **$0** (free tier)
- After free tier: **~$9-12/month**

### Monitoring

**View logs:**
```bash
# SSH into the server, then:
docker logs -f harbor-relay

# Or via SSM:
journalctl -u libp2p-relay -f
```

**Check relay status:**
```bash
systemctl status libp2p-relay
```

### Troubleshooting

**Relay not starting:**
```bash
# Check Docker status
systemctl status docker

# Check relay service status
systemctl status libp2p-relay

# View detailed logs
journalctl -u libp2p-relay -n 100
```

**Can't connect from Harbor:**
1. Verify security group allows inbound on port 4001 (TCP and UDP)
2. Check that the peer ID in your multiaddress is correct
3. Ensure the relay container is running: `docker ps`

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
│  │  │  │  │   Docker: libp2p-relay-daemon   │  │  │  │  │
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
