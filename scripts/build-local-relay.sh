# /bin/bash
cd /home/bakobi/repos/harbor/relay-server
RUST_LOG=debug ./target/release/harbor-relay \
  --port 4001 \
  --announce-ip 154.5.126.219 \
  --identity-key-path ~/.config/harbor-relay/id.key

