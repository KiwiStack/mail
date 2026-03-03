#!/usr/bin/env bash
set -euo pipefail

# Kiwi Mail — LXC Provisioning Script
# Creates a Proxmox LXC container with Stalwart Mail Server + Kiwi Mail wrapper

VMID=111
HOSTNAME="kiwi-mail"
IP="10.10.10.111"
BRIDGE="vmbr1"
GATEWAY="10.10.10.1"
PROXMOX_HOST="root@10.10.10.1"
STALWART_VERSION="0.10.5"
TEMPLATE="local:vztmpl/ubuntu-24.04-standard_24.04-2_amd64.tar.zst"

echo "==> Creating LXC container ${VMID} (${HOSTNAME})"
ssh "${PROXMOX_HOST}" pct create "${VMID}" "${TEMPLATE}" \
  --hostname "${HOSTNAME}" \
  --cores 2 \
  --memory 1024 \
  --swap 512 \
  --rootfs "local:8" \
  --net0 "name=eth0,bridge=${BRIDGE},ip=${IP}/24,gw=${GATEWAY}" \
  --nameserver 1.1.1.1 \
  --features nesting=1 \
  --unprivileged 1 \
  --onboot 1 \
  --start 1

echo "==> Waiting for container to start..."
sleep 5

echo "==> Installing Stalwart Mail Server v${STALWART_VERSION}"
ssh "${PROXMOX_HOST}" pct exec "${VMID}" -- bash -c '
set -euo pipefail

apt-get update -qq
apt-get install -y -qq curl ca-certificates

# Create directories
mkdir -p /opt/upstream/bin /opt/upstream/etc /var/lib/upstream/db /var/lib/upstream/blobs /opt/kiwi/bin /etc/kiwi

# Download Stalwart binary
STALWART_VERSION="'"${STALWART_VERSION}"'"
cd /tmp
curl -Lo stalwart.tar.gz \
  "https://github.com/stalwartlabs/stalwart/releases/download/v${STALWART_VERSION}/stalwart-mail-x86_64-unknown-linux-gnu.tar.gz"
tar xzf stalwart.tar.gz
mv stalwart-mail /opt/upstream/bin/
chmod +x /opt/upstream/bin/stalwart-mail
rm stalwart.tar.gz
'

echo "==> Writing Stalwart config"
ssh "${PROXMOX_HOST}" pct exec "${VMID}" -- bash -c 'cat > /opt/upstream/etc/config.toml << '\''STALWART_EOF'\''
[server]
hostname = "mail.kiwi.local"

[server.listener.jmap]
bind = ["127.0.0.1:8080"]
protocol = "http"

[server.listener.smtp]
bind = ["127.0.0.1:25"]
protocol = "smtp"

[storage]
data = "rocksdb"
blob = "fs"
fts = "rocksdb"
lookup = "rocksdb"
directory = "internal"

[store.rocksdb]
type = "rocksdb"
path = "/var/lib/upstream/db"

[store.fs]
type = "fs"
path = "/var/lib/upstream/blobs"

[directory.internal]
type = "internal"
store = "rocksdb"

[tracer.stdout]
type = "stdout"
level = "info"

[authentication.fallback-admin]
user = "admin"
secret = "changeme"
STALWART_EOF'

echo "==> Writing Kiwi Mail config"
ssh "${PROXMOX_HOST}" pct exec "${VMID}" -- bash -c 'cat > /etc/kiwi/config.toml << '\''KIWI_EOF'\''
listen_addr = "0.0.0.0:8443"
upstream_addr = "http://127.0.0.1:8080"
upstream_bin = "/opt/upstream/bin/stalwart-mail"
upstream_config = "/opt/upstream/etc/config.toml"
health_check_interval = "5s"
health_check_timeout = "30s"
log_level = "info"
admin_user = "admin"
admin_pass = "changeme"
KIWI_EOF'

echo "==> Creating systemd service"
ssh "${PROXMOX_HOST}" pct exec "${VMID}" -- bash -c 'cat > /etc/systemd/system/kiwi-mail.service << '\''SYSTEMD_EOF'\''
[Unit]
Description=Kiwi Mail Wrapper
After=network.target

[Service]
Type=simple
ExecStart=/opt/kiwi/bin/kiwi-mail
Environment=KIWI_CONFIG=/etc/kiwi/config.toml
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
SYSTEMD_EOF

systemctl daemon-reload
systemctl enable kiwi-mail'

echo "==> LXC ${VMID} provisioned successfully"
echo "    IP: ${IP}"
echo "    Stalwart: http://127.0.0.1:8080 (inside LXC)"
echo "    Kiwi Mail: http://${IP}:8443 (from network)"
echo ""
echo "Next steps:"
echo "  1. Build the wrapper: cargo build --release"
echo "  2. Copy binary: scp target/release/kiwi-mail root@${IP}:/opt/kiwi/bin/"
echo "  3. Start service: ssh root@${IP} systemctl start kiwi-mail"
