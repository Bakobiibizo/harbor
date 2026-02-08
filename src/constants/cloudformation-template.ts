// CloudFormation templates for deploying Harbor relay servers on AWS
// These templates are embedded in the app for easy copy-paste deployment

export const RELAY_CLOUDFORMATION_TEMPLATE = `AWSTemplateFormatVersion: "2010-09-09"
Description: "Harbor libp2p Relay Server - Enables NAT traversal for Harbor chat app users"

Metadata:
  AWS::CloudFormation::Interface:
    ParameterGroups:
      - Label:
          default: "Instance Configuration"
        Parameters:
          - InstanceType
          - KeyPairName
      - Label:
          default: "Relay Configuration"
        Parameters:
          - RelayPort
          - MaxReservations
          - MaxCircuits

Parameters:
  InstanceType:
    Type: String
    Default: t2.micro
    AllowedValues:
      - t2.micro
      - t3.micro
      - t2.small
      - t3.small
    Description: "EC2 instance type. t2.micro and t3.micro are free tier eligible (750 hours/month)"

  KeyPairName:
    Type: String
    Default: ""
    Description: "(Optional) EC2 key pair name for SSH access. Leave empty to disable SSH."

  RelayPort:
    Type: Number
    Default: 4001
    MinValue: 1024
    MaxValue: 65535
    Description: "Port for the libp2p relay server"

  MaxReservations:
    Type: Number
    Default: 128
    Description: "Maximum number of relay reservations (peers that can use this relay)"

  MaxCircuits:
    Type: Number
    Default: 16
    Description: "Maximum number of active relay circuits per peer"

Conditions:
  HasKeyPair: !Not [!Equals [!Ref KeyPairName, ""]]

Resources:
  # VPC for the relay server
  RelayVPC:
    Type: AWS::EC2::VPC
    Properties:
      CidrBlock: 10.0.0.0/16
      EnableDnsSupport: true
      EnableDnsHostnames: true
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-vpc"

  # Internet Gateway
  InternetGateway:
    Type: AWS::EC2::InternetGateway
    Properties:
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-igw"

  VPCGatewayAttachment:
    Type: AWS::EC2::VPCGatewayAttachment
    Properties:
      VpcId: !Ref RelayVPC
      InternetGatewayId: !Ref InternetGateway

  # Public Subnet
  PublicSubnet:
    Type: AWS::EC2::Subnet
    Properties:
      VpcId: !Ref RelayVPC
      CidrBlock: 10.0.1.0/24
      MapPublicIpOnLaunch: true
      AvailabilityZone: !Select [0, !GetAZs ""]
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-public-subnet"

  # Route Table
  PublicRouteTable:
    Type: AWS::EC2::RouteTable
    Properties:
      VpcId: !Ref RelayVPC
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-public-rt"

  PublicRoute:
    Type: AWS::EC2::Route
    DependsOn: VPCGatewayAttachment
    Properties:
      RouteTableId: !Ref PublicRouteTable
      DestinationCidrBlock: 0.0.0.0/0
      GatewayId: !Ref InternetGateway

  SubnetRouteTableAssociation:
    Type: AWS::EC2::SubnetRouteTableAssociation
    Properties:
      SubnetId: !Ref PublicSubnet
      RouteTableId: !Ref PublicRouteTable

  # Security Group
  RelaySecurityGroup:
    Type: AWS::EC2::SecurityGroup
    Properties:
      GroupDescription: !Sub "Security group for \${AWS::StackName} Harbor relay server"
      VpcId: !Ref RelayVPC
      SecurityGroupIngress:
        # libp2p relay port (TCP)
        - IpProtocol: tcp
          FromPort: !Ref RelayPort
          ToPort: !Ref RelayPort
          CidrIp: 0.0.0.0/0
          Description: libp2p relay TCP
        # libp2p relay port (UDP for QUIC)
        - IpProtocol: udp
          FromPort: !Ref RelayPort
          ToPort: !Ref RelayPort
          CidrIp: 0.0.0.0/0
          Description: libp2p relay UDP/QUIC
        # SSH access (only if key pair provided)
        - IpProtocol: tcp
          FromPort: 22
          ToPort: 22
          CidrIp: 0.0.0.0/0
          Description: SSH access
      SecurityGroupEgress:
        - IpProtocol: -1
          CidrIp: 0.0.0.0/0
          Description: Allow all outbound
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-sg"

  # SSM Parameter to store the relay address
  RelayAddressParameter:
    Type: AWS::SSM::Parameter
    Properties:
      Name: !Sub "/harbor/\${AWS::StackName}/relay-address"
      Type: String
      Value: "Starting up... check back in 2 minutes"
      Description: !Sub "Full relay address for Harbor (\${AWS::StackName}) - copy this into the app"
      Tags:
        Application: harbor-chat

  # IAM Role for EC2
  RelayInstanceRole:
    Type: AWS::IAM::Role
    Properties:
      AssumeRolePolicyDocument:
        Version: "2012-10-17"
        Statement:
          - Effect: Allow
            Principal:
              Service: ec2.amazonaws.com
            Action: sts:AssumeRole
      ManagedPolicyArns:
        - arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore
      Policies:
        - PolicyName: WriteRelayAddress
          PolicyDocument:
            Version: "2012-10-17"
            Statement:
              - Effect: Allow
                Action:
                  - ssm:PutParameter
                Resource: !Sub "arn:aws:ssm:\${AWS::Region}:\${AWS::AccountId}:parameter/harbor/\${AWS::StackName}/relay-address"
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-role"

  RelayInstanceProfile:
    Type: AWS::IAM::InstanceProfile
    Properties:
      Roles:
        - !Ref RelayInstanceRole

  # Elastic IP for stable address
  RelayEIP:
    Type: AWS::EC2::EIP
    Properties:
      Domain: vpc
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-eip"

  EIPAssociation:
    Type: AWS::EC2::EIPAssociation
    Properties:
      InstanceId: !Ref RelayInstance
      AllocationId: !GetAtt RelayEIP.AllocationId

  # EC2 Instance
  RelayInstance:
    Type: AWS::EC2::Instance
    Properties:
      InstanceType: !Ref InstanceType
      ImageId: !Sub "{{resolve:ssm:/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64}}"
      KeyName: !If [HasKeyPair, !Ref KeyPairName, !Ref "AWS::NoValue"]
      IamInstanceProfile: !Ref RelayInstanceProfile
      NetworkInterfaces:
        - AssociatePublicIpAddress: true
          DeviceIndex: 0
          GroupSet:
            - !Ref RelaySecurityGroup
          SubnetId: !Ref PublicSubnet
      BlockDeviceMappings:
        - DeviceName: /dev/xvda
          Ebs:
            VolumeSize: 8
            VolumeType: gp3
            Encrypted: true
      UserData:
        Fn::Base64: !Sub |
          #!/bin/bash -xe
          exec > >(tee /var/log/user-data.log|logger -t user-data -s 2>/dev/console) 2>&1

          echo "=== Starting Harbor Relay Setup ==="
          echo "Region: \${AWS::Region}"
          echo "Stack: \${AWS::StackName}"
          echo "RelayPort: \${RelayPort}"

          EXPECTED_SHA256="c5dcb143d69558107ced27a0dfd30542a88a69906aa53404d9b31ef97a1c66a3"
          SERVICE_NAME="\${AWS::StackName}"
          BINARY_URL="https://github.com/bakobiibizo/harbor/raw/main/relay-server/bin/harbor-relay"

          # Download pre-compiled binary
          echo "Downloading pre-compiled relay binary..."
          curl -L -o /usr/local/bin/$SERVICE_NAME "$BINARY_URL"

          # Verify SHA256 hash
          echo "Verifying binary integrity..."
          ACTUAL_SHA256=$(sha256sum /usr/local/bin/$SERVICE_NAME | cut -d ' ' -f 1)
          if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
            echo "ERROR: SHA256 mismatch!"
            echo "  Expected: $EXPECTED_SHA256"
            echo "  Got:      $ACTUAL_SHA256"
            exit 1
          fi
          echo "SHA256 verified OK"

          chmod +x /usr/local/bin/$SERVICE_NAME

          # The relay binary auto-generates an identity key on first run.
          # This identity persists across service restarts.
          mkdir -p /root/.config/$SERVICE_NAME

          # Get public IP using IMDSv2
          echo "Getting public IP..."
          IMDS_TOKEN=$(curl -X PUT "http://169.254.169.254/latest/api/token" -H "X-aws-ec2-metadata-token-ttl-seconds: 21600" 2>/dev/null)
          PUBLIC_IP=$(curl -H "X-aws-ec2-metadata-token: $IMDS_TOKEN" http://169.254.169.254/latest/meta-data/public-ipv4 2>/dev/null)

          if [ -z "$PUBLIC_IP" ] || echo "$PUBLIC_IP" | grep -q "<?xml"; then
            echo "IMDSv2 failed, trying external IP service..."
            PUBLIC_IP=$(curl -s https://checkip.amazonaws.com 2>/dev/null | tr -d '\\n')
          fi
          echo "Public IP: $PUBLIC_IP"

          # Start relay without announce-ip so it generates its identity
          echo "Creating initial systemd service..."
          cat > /etc/systemd/system/$SERVICE_NAME.service << SERVICEEOF
          [Unit]
          Description=Harbor Relay Server ($SERVICE_NAME)
          After=network-online.target
          Wants=network-online.target

          [Service]
          Type=simple
          Restart=always
          RestartSec=10
          Environment=RUST_LOG=info
          ExecStart=/usr/local/bin/$SERVICE_NAME --port \${RelayPort} --max-reservations \${MaxReservations} --max-circuits-per-peer \${MaxCircuits}
          StandardOutput=journal
          StandardError=journal

          [Install]
          WantedBy=multi-user.target
          SERVICEEOF

          systemctl daemon-reload
          systemctl enable $SERVICE_NAME
          systemctl start $SERVICE_NAME

          echo "Waiting for relay to start..."
          sleep 10
          PEER_ID=""
          for i in {1..30}; do
            PEER_ID=$(journalctl -u $SERVICE_NAME --no-pager 2>&1 | grep -oE '(12D3KooW|Qm)[a-zA-Z0-9]+' | head -1)
            if [ -n "$PEER_ID" ]; then
              echo "Found Peer ID: $PEER_ID"
              break
            fi
            sleep 2
          done

          # Wait for Elastic IP to be associated (happens after UserData)
          echo "Waiting for Elastic IP association..."
          PREV_IP=""
          STABLE_COUNT=0
          for attempt in {1..60}; do
            CURRENT_IP=$(curl -s https://checkip.amazonaws.com 2>/dev/null | tr -d '\\n')
            if [ "$CURRENT_IP" = "$PREV_IP" ] && [ -n "$CURRENT_IP" ]; then
              STABLE_COUNT=$((STABLE_COUNT + 1))
            else
              STABLE_COUNT=0
            fi
            PREV_IP="$CURRENT_IP"
            if [ "$STABLE_COUNT" -ge 3 ]; then
              echo "Public IP stabilized at $CURRENT_IP"
              break
            fi
            sleep 5
          done
          PUBLIC_IP="$CURRENT_IP"
          echo "Public IP: $PUBLIC_IP"

          # Rewrite service with correct announce-ip and restart
          cat > /etc/systemd/system/$SERVICE_NAME.service << SERVICEEOF
          [Unit]
          Description=Harbor Relay Server ($SERVICE_NAME)
          After=network-online.target
          Wants=network-online.target

          [Service]
          Type=simple
          Restart=always
          RestartSec=10
          Environment=RUST_LOG=info
          ExecStart=/usr/local/bin/$SERVICE_NAME --port \${RelayPort} --announce-ip $PUBLIC_IP --max-reservations \${MaxReservations} --max-circuits-per-peer \${MaxCircuits}
          StandardOutput=journal
          StandardError=journal

          [Install]
          WantedBy=multi-user.target
          SERVICEEOF

          systemctl daemon-reload
          systemctl restart $SERVICE_NAME
          sleep 5

          SSM_PARAM_NAME="/harbor/\${AWS::StackName}/relay-address"

          if [ -n "$PEER_ID" ] && [ -n "$PUBLIC_IP" ]; then
            RELAY_ADDRESS="/ip4/$PUBLIC_IP/tcp/\${RelayPort}/p2p/$PEER_ID"
            echo "Relay address: $RELAY_ADDRESS"

            aws ssm put-parameter \\
              --name "$SSM_PARAM_NAME" \\
              --value "$RELAY_ADDRESS" \\
              --type String \\
              --overwrite \\
              --region \${AWS::Region}

            echo "=== SUCCESS ==="
            echo "YOUR RELAY ADDRESS (copy this to Harbor):"
            echo "$RELAY_ADDRESS"
          else
            echo "=== PARTIAL FAILURE ==="
            echo "Public IP: $PUBLIC_IP"
            echo "Peer ID: $PEER_ID"

            aws ssm put-parameter \\
              --name "$SSM_PARAM_NAME" \\
              --value "ERROR: Setup incomplete. Check /var/log/user-data.log" \\
              --type String \\
              --overwrite \\
              --region \${AWS::Region}
          fi

          echo "=== Setup Complete ==="

      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-server"
        - Key: Application
          Value: harbor-chat

Outputs:
  Step1WaitTwoMinutes:
    Description: "STEP 1: Wait ~2 minutes for the server to start"
    Value: "The relay generates a unique identity on first boot. Allow a couple of minutes for startup."

  Step2GetYourRelayAddress:
    Description: "STEP 2: Click this link to get your relay address"
    Value: !Sub "https://\${AWS::Region}.console.aws.amazon.com/systems-manager/parameters/harbor/\${AWS::StackName}/relay-address/description?region=\${AWS::Region}"

  Step3CopyRelayAddress:
    Description: "STEP 3: Copy the 'Value' field and paste it into Harbor"
    Value: "On that page, find the 'Value' field. It looks like: /ip4/1.2.3.4/tcp/4001/p2p/12D3KooW..."

  RelayPublicIP:
    Description: "Public IP address (for reference)"
    Value: !Ref RelayEIP

  RelayPeerID:
    Description: "Relay Peer ID (auto-generated on first boot, persists across restarts)"
    Value: "Check SSM Parameter Store for the full relay address including Peer ID"

  EstimatedMonthlyCost:
    Description: "Cost: FREE for 12 months, then ~$9/month"
    Value: "AWS Free Tier: 750 hours/month for first year"
`;

export const COMMUNITY_RELAY_CLOUDFORMATION_TEMPLATE = `AWSTemplateFormatVersion: "2010-09-09"
Description: "Harbor Community Relay Server - NAT traversal + community boards with SQLite storage"

Metadata:
  AWS::CloudFormation::Interface:
    ParameterGroups:
      - Label:
          default: "Instance Configuration"
        Parameters:
          - InstanceType
          - KeyPairName
      - Label:
          default: "Relay Configuration"
        Parameters:
          - RelayPort
          - MaxReservations
          - MaxCircuits
      - Label:
          default: "Community Configuration"
        Parameters:
          - CommunityName
          - RateLimitMaxRequests
          - RateLimitWindowSecs

Parameters:
  InstanceType:
    Type: String
    Default: t2.micro
    AllowedValues:
      - t2.micro
      - t3.micro
      - t2.small
      - t3.small
    Description: "EC2 instance type. t2.micro and t3.micro are free tier eligible (750 hours/month)"

  KeyPairName:
    Type: String
    Default: ""
    Description: "(Optional) EC2 key pair name for SSH access. Leave empty to disable SSH."

  RelayPort:
    Type: Number
    Default: 4001
    MinValue: 1024
    MaxValue: 65535
    Description: "Port for the libp2p relay server (TCP + UDP/QUIC)"

  MaxReservations:
    Type: Number
    Default: 128
    Description: "Maximum number of relay reservations (peers that can use this relay)"

  MaxCircuits:
    Type: Number
    Default: 16
    Description: "Maximum number of active relay circuits per peer"

  CommunityName:
    Type: String
    Default: "Harbor Community"
    Description: "Name for your community (shown to peers who join)"

  RateLimitMaxRequests:
    Type: Number
    Default: 60
    Description: "Maximum board sync requests per peer within the rate limit window"

  RateLimitWindowSecs:
    Type: Number
    Default: 60
    Description: "Rate limit window duration in seconds"

Conditions:
  HasKeyPair: !Not [!Equals [!Ref KeyPairName, ""]]

Resources:
  RelayVPC:
    Type: AWS::EC2::VPC
    Properties:
      CidrBlock: 10.0.0.0/16
      EnableDnsSupport: true
      EnableDnsHostnames: true
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-vpc"

  InternetGateway:
    Type: AWS::EC2::InternetGateway
    Properties:
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-igw"

  VPCGatewayAttachment:
    Type: AWS::EC2::VPCGatewayAttachment
    Properties:
      VpcId: !Ref RelayVPC
      InternetGatewayId: !Ref InternetGateway

  PublicSubnet:
    Type: AWS::EC2::Subnet
    Properties:
      VpcId: !Ref RelayVPC
      CidrBlock: 10.0.1.0/24
      MapPublicIpOnLaunch: true
      AvailabilityZone: !Select [0, !GetAZs ""]
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-public-subnet"

  PublicRouteTable:
    Type: AWS::EC2::RouteTable
    Properties:
      VpcId: !Ref RelayVPC
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-public-rt"

  PublicRoute:
    Type: AWS::EC2::Route
    DependsOn: VPCGatewayAttachment
    Properties:
      RouteTableId: !Ref PublicRouteTable
      DestinationCidrBlock: 0.0.0.0/0
      GatewayId: !Ref InternetGateway

  SubnetRouteTableAssociation:
    Type: AWS::EC2::SubnetRouteTableAssociation
    Properties:
      SubnetId: !Ref PublicSubnet
      RouteTableId: !Ref PublicRouteTable

  RelaySecurityGroup:
    Type: AWS::EC2::SecurityGroup
    Properties:
      GroupDescription: !Sub "Security group for \${AWS::StackName} Harbor community relay"
      VpcId: !Ref RelayVPC
      SecurityGroupIngress:
        - IpProtocol: tcp
          FromPort: !Ref RelayPort
          ToPort: !Ref RelayPort
          CidrIp: 0.0.0.0/0
          Description: libp2p relay TCP
        - IpProtocol: udp
          FromPort: !Ref RelayPort
          ToPort: !Ref RelayPort
          CidrIp: 0.0.0.0/0
          Description: libp2p relay UDP/QUIC
      SecurityGroupEgress:
        - IpProtocol: -1
          CidrIp: 0.0.0.0/0
          Description: Allow all outbound
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-sg"

  SSHIngressRule:
    Type: AWS::EC2::SecurityGroupIngress
    Condition: HasKeyPair
    Properties:
      GroupId: !Ref RelaySecurityGroup
      IpProtocol: tcp
      FromPort: 22
      ToPort: 22
      CidrIp: 0.0.0.0/0
      Description: SSH access (only when key pair is provided)

  RelayAddressParameter:
    Type: AWS::SSM::Parameter
    Properties:
      Name: !Sub "/harbor/\${AWS::StackName}/relay-address"
      Type: String
      Value: "Starting up... check back in 2 minutes"
      Description: !Sub "Full relay address for Harbor (\${AWS::StackName}) - copy this into the app"
      Tags:
        Application: harbor-chat

  RelayInstanceRole:
    Type: AWS::IAM::Role
    Properties:
      AssumeRolePolicyDocument:
        Version: "2012-10-17"
        Statement:
          - Effect: Allow
            Principal:
              Service: ec2.amazonaws.com
            Action: sts:AssumeRole
      ManagedPolicyArns:
        - arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore
      Policies:
        - PolicyName: WriteRelayAddress
          PolicyDocument:
            Version: "2012-10-17"
            Statement:
              - Effect: Allow
                Action:
                  - ssm:PutParameter
                Resource: !Sub "arn:aws:ssm:\${AWS::Region}:\${AWS::AccountId}:parameter/harbor/\${AWS::StackName}/relay-address"
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-role"

  RelayInstanceProfile:
    Type: AWS::IAM::InstanceProfile
    Properties:
      Roles:
        - !Ref RelayInstanceRole

  RelayEIP:
    Type: AWS::EC2::EIP
    Properties:
      Domain: vpc
      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-eip"

  EIPAssociation:
    Type: AWS::EC2::EIPAssociation
    Properties:
      InstanceId: !Ref RelayInstance
      AllocationId: !GetAtt RelayEIP.AllocationId

  RelayInstance:
    Type: AWS::EC2::Instance
    Properties:
      InstanceType: !Ref InstanceType
      ImageId: !Sub "{{resolve:ssm:/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64}}"
      KeyName: !If [HasKeyPair, !Ref KeyPairName, !Ref "AWS::NoValue"]
      IamInstanceProfile: !Ref RelayInstanceProfile
      NetworkInterfaces:
        - AssociatePublicIpAddress: true
          DeviceIndex: 0
          GroupSet:
            - !Ref RelaySecurityGroup
          SubnetId: !Ref PublicSubnet
      BlockDeviceMappings:
        - DeviceName: /dev/xvda
          Ebs:
            VolumeSize: 20
            VolumeType: gp3
            Encrypted: true
      UserData:
        Fn::Base64: !Sub |
          #!/bin/bash -xe
          exec > >(tee /var/log/user-data.log|logger -t user-data -s 2>/dev/console) 2>&1

          echo "=== Starting Harbor Community Relay Setup ==="
          echo "Region: \${AWS::Region}"
          echo "Stack: \${AWS::StackName}"
          echo "RelayPort: \${RelayPort}"
          echo "CommunityName: \${CommunityName}"

          EXPECTED_SHA256="9425ab1d055fa3ceb4f2cc6eba6b29571e12fe2da868f305374d000d3892a05d"
          SERVICE_NAME="\${AWS::StackName}"
          BINARY_URL="https://github.com/bakobiibizo/harbor/raw/main/relay-server/bin/harbor-relay"

          # Download pre-compiled binary
          echo "Downloading pre-compiled relay binary..."
          curl -L -o /usr/local/bin/$SERVICE_NAME "$BINARY_URL"

          # Verify SHA256 hash
          echo "Verifying binary integrity..."
          ACTUAL_SHA256=$(sha256sum /usr/local/bin/$SERVICE_NAME | cut -d ' ' -f 1)
          if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
            echo "ERROR: SHA256 mismatch!"
            echo "  Expected: $EXPECTED_SHA256"
            echo "  Got:      $ACTUAL_SHA256"
            exit 1
          fi
          echo "SHA256 verified OK"

          chmod +x /usr/local/bin/$SERVICE_NAME

          mkdir -p /root/.config/$SERVICE_NAME
          mkdir -p /var/lib/$SERVICE_NAME/data

          # Start relay without announce-ip so it generates its identity
          echo "Creating initial systemd service..."
          cat > /etc/systemd/system/$SERVICE_NAME.service << SERVICEEOF
          [Unit]
          Description=Harbor Community Relay Server ($SERVICE_NAME)
          After=network-online.target
          Wants=network-online.target

          [Service]
          Type=simple
          Restart=always
          RestartSec=10
          Environment=RUST_LOG=info
          ExecStart=/usr/local/bin/$SERVICE_NAME --port \${RelayPort} --max-reservations \${MaxReservations} --max-circuits-per-peer \${MaxCircuits} --community --community-name "\${CommunityName}" --data-dir /var/lib/$SERVICE_NAME/data --rate-limit-max-requests \${RateLimitMaxRequests} --rate-limit-window-secs \${RateLimitWindowSecs}
          StandardOutput=journal
          StandardError=journal

          [Install]
          WantedBy=multi-user.target
          SERVICEEOF

          systemctl daemon-reload
          systemctl enable $SERVICE_NAME
          systemctl start $SERVICE_NAME

          echo "Waiting for relay to start..."
          sleep 10
          PEER_ID=""
          for i in {1..30}; do
            PEER_ID=$(journalctl -u $SERVICE_NAME --no-pager 2>&1 | grep -oE '(12D3KooW|Qm)[a-zA-Z0-9]+' | head -1)
            if [ -n "$PEER_ID" ]; then
              echo "Found Peer ID: $PEER_ID"
              break
            fi
            sleep 2
          done

          # Wait for Elastic IP to be associated (happens after UserData)
          echo "Waiting for Elastic IP association..."
          PREV_IP=""
          STABLE_COUNT=0
          for attempt in {1..60}; do
            CURRENT_IP=$(curl -s https://checkip.amazonaws.com 2>/dev/null | tr -d '\\n')
            if [ "$CURRENT_IP" = "$PREV_IP" ] && [ -n "$CURRENT_IP" ]; then
              STABLE_COUNT=$((STABLE_COUNT + 1))
            else
              STABLE_COUNT=0
            fi
            PREV_IP="$CURRENT_IP"
            if [ "$STABLE_COUNT" -ge 3 ]; then
              echo "Public IP stabilized at $CURRENT_IP"
              break
            fi
            sleep 5
          done
          PUBLIC_IP="$CURRENT_IP"
          echo "Public IP: $PUBLIC_IP"

          # Rewrite service with correct announce-ip and restart
          cat > /etc/systemd/system/$SERVICE_NAME.service << SERVICEEOF
          [Unit]
          Description=Harbor Community Relay Server ($SERVICE_NAME)
          After=network-online.target
          Wants=network-online.target

          [Service]
          Type=simple
          Restart=always
          RestartSec=10
          Environment=RUST_LOG=info
          ExecStart=/usr/local/bin/$SERVICE_NAME --port \${RelayPort} --announce-ip $PUBLIC_IP --max-reservations \${MaxReservations} --max-circuits-per-peer \${MaxCircuits} --community --community-name "\${CommunityName}" --data-dir /var/lib/$SERVICE_NAME/data --rate-limit-max-requests \${RateLimitMaxRequests} --rate-limit-window-secs \${RateLimitWindowSecs}
          StandardOutput=journal
          StandardError=journal

          [Install]
          WantedBy=multi-user.target
          SERVICEEOF

          systemctl daemon-reload
          systemctl restart $SERVICE_NAME
          sleep 5

          SSM_PARAM_NAME="/harbor/\${AWS::StackName}/relay-address"

          if [ -n "$PEER_ID" ] && [ -n "$PUBLIC_IP" ]; then
            RELAY_ADDRESS="/ip4/$PUBLIC_IP/tcp/\${RelayPort}/p2p/$PEER_ID"
            echo "Relay address: $RELAY_ADDRESS"

            aws ssm put-parameter \\
              --name "$SSM_PARAM_NAME" \\
              --value "$RELAY_ADDRESS" \\
              --type String \\
              --overwrite \\
              --region \${AWS::Region}

            echo "=== SUCCESS ==="
            echo "$RELAY_ADDRESS"
            echo "Community: \${CommunityName}"
          else
            echo "=== PARTIAL FAILURE ==="
            echo "Public IP: $PUBLIC_IP"
            echo "Peer ID: $PEER_ID"

            aws ssm put-parameter \\
              --name "$SSM_PARAM_NAME" \\
              --value "ERROR: Setup incomplete. Check /var/log/user-data.log" \\
              --type String \\
              --overwrite \\
              --region \${AWS::Region}
          fi

          echo "=== Setup Complete ==="

      Tags:
        - Key: Name
          Value: !Sub "\${AWS::StackName}-server"
        - Key: Application
          Value: harbor-chat

Outputs:
  Step1WaitTwoMinutes:
    Description: "STEP 1: Wait ~2 minutes for the server to start"
    Value: "The relay downloads a pre-compiled binary and generates a unique identity on first boot."

  Step2GetYourRelayAddress:
    Description: "STEP 2: Click this link to get your relay address"
    Value: !Sub "https://\${AWS::Region}.console.aws.amazon.com/systems-manager/parameters/harbor/\${AWS::StackName}/relay-address/description?region=\${AWS::Region}"

  Step3CopyRelayAddress:
    Description: "STEP 3: Copy the 'Value' field and paste it into Harbor"
    Value: "On that page, find the 'Value' field. It looks like: /ip4/1.2.3.4/tcp/4001/p2p/12D3KooW..."

  CommunityNameOutput:
    Description: "Community name"
    Value: !Ref CommunityName

  RelayPublicIP:
    Description: "Public IP address (for reference)"
    Value: !Ref RelayEIP

  RelayPeerID:
    Description: "Relay Peer ID (auto-generated on first boot, persists across restarts)"
    Value: "Check SSM Parameter Store for the full relay address including Peer ID"

  EstimatedMonthlyCost:
    Description: "Cost: FREE for 12 months, then ~$9/month"
    Value: "AWS Free Tier: 750 hours/month for first year"
`;
