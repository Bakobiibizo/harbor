#!/usr/bin/env bash

SHA256=$(sha256sum "relay-server/bin/harbor-relay" | awk '{print $1}')
echo $SHA256 "relay-server/bin/harbor-relay" > "relay-server/bin/harbor-relay.sha256"
