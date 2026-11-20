#!/usr/bin/env bash
# Automated GitHub Issue creation and synchronization script for Waymesh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="sahalhes/waymesh"

echo "=== Waymesh Community Issues Sync ==="
if command -v python3 >/dev/null 2>&1; then
    echo "Running Python issue creator and roadmap synchronizer..."
    python3 "$SCRIPT_DIR/sync_community_issues.py"
else
    echo "Error: Python 3 is required to run sync_community_issues.py."
    exit 1
fi
