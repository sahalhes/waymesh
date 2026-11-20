#!/usr/bin/env bash
set -euo pipefail

echo "=========================================================="
echo "    Waymesh VPS Setup for Contabo (waymesh.icvalley.tech)  "
echo "=========================================================="

# 1. Update and install prerequisites
echo "[*] Updating system packages..."
sudo apt-get update -y
sudo apt-get install -y curl ufw git ca-certificates gnupg

# 2. Install Docker if not present
if ! command -v docker &> /dev/null; then
    echo "[*] Installing Docker Engine..."
    curl -fsSL https://get.docker.com -o get-docker.sh
    sudo sh get-docker.sh
    sudo usermod -aG docker "$USER" || true
    rm -f get-docker.sh
fi

# 3. Configure UFW Firewall (Strictly allow SSH, HTTP 80, HTTPS 443)
echo "[*] Hardening VPS Firewall (UFW)..."
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 22/tcp comment 'SSH'
sudo ufw allow 80/tcp comment 'HTTP Caddy'
sudo ufw allow 443/tcp comment 'HTTPS Caddy'
sudo ufw --force enable

# 4. Prepare environment configuration
cd "$(dirname "$0")"
if [ ! -f .env ]; then
    echo "[*] Initializing .env configuration..."
    cp .env.example .env
fi

# 5. Boot production Docker Compose stack
echo "[*] Launching Waymesh Node + PostGIS + Caddy (Auto-SSL)..."
docker compose -f docker-compose.prod.yml up -d --build

echo ""
echo "=========================================================="
echo "✔ Waymesh is running on your Contabo VPS!"
echo "Status check:  docker compose -f docker-compose.prod.yml ps"
echo "Live Logs:     docker compose -f docker-compose.prod.yml logs -f"
echo "Health Endpoint: https://waymesh.icvalley.tech/health"
echo "=========================================================="
