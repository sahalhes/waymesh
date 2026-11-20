# Waymesh Claude Plugin

Official [Claude Code](https://code.claude.com/docs/en/plugins/overview) and [Anthropic Directory](https://claude.ai/directory) plugin for the **Waymesh Decentralized Mobility and Logistics Network**.

Enables Claude to search for peer rides, coordinate safe carpooling (with strict same-gender separation), publish travel corridors, book parcel crowdshipping, and verify physical pickups and deliveries using cryptographically validated 6-digit OTP codes.

---

## What This Plugin Provides

1. **Remote MCP Server (`.mcp.json`)**:
   Direct streamable HTTP connection to the live Waymesh node at `https://waymesh.icvalley.tech/mcp` providing:
   - `waymesh_search_matches`
   - `waymesh_publish_journey`
   - `waymesh_create_booking`
   - `waymesh_accept_booking`
   - `waymesh_verify_handover`

2. **Mobility Skill (`skills/waymesh-rideshare/SKILL.md`)**:
   Autonomous skill that activates whenever a user mentions rides, carpools, parcels, or commuting:
   - **Safe Carpooling (Strict Gender Separation)**: Female passengers strictly match with female drivers; male passengers strictly match with male drivers.
   - **Zero-UUID Conversational Provisioning**: Pass friendly names ("Priya", "CET Student") without needing raw cryptographic hashes.
   - **Landmark Geocoding**: Automatically resolves local college campuses, tech parks, and transit hubs.
   - **Verifiable OTP Handover**: 6-digit secret codes for passenger boarding and parcel custody verification.
   - **Private Peer-to-Peer Settlement**: Settle directly between parties privately in Indian Rupees (₹), direct peer UPI, or offline physical cash. Strictly no crypto / no Web3 tokens.

---

## Installation & Usage

### Method 1: Run Locally in Claude Code (Zero Install)

Test the plugin immediately without modifying persistent settings:

```bash
claude --plugin-dir /path/to/waymesh/connectors/claude/waymesh-plugin
```

Once inside Claude Code, run `/help` or invoke the skill directly:
```text
/waymesh-mobility:waymesh-rideshare
```
Or simply converse naturally:
> *"I need safe carpooling from CET to Kochuveli Railway Station at 5:00 PM today. Female passenger."*

### Method 2: Install via Waymesh Marketplace

Add the Waymesh GitHub repository as a plugin marketplace:

```bash
# Add official Waymesh marketplace
claude plugin marketplace add sahalhes/waymesh

# Install the plugin
claude plugin install waymesh-mobility
```

To install from a local clone of the repository:
```bash
claude plugin marketplace add ./connectors/claude
claude plugin install waymesh-mobility
```

### Method 3: Use with Claude Desktop

Add the streamable HTTP MCP server to your `claude_desktop_config.json`:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "waymesh": {
      "type": "http",
      "url": "https://waymesh.icvalley.tech/mcp"
    }
  }
}
```

Or run via the local stdio binary:
```json
{
  "mcpServers": {
    "waymesh": {
      "command": "cargo",
      "args": ["run", "-p", "waymesh-mcp", "--quiet"]
    }
  }
}
```

---

## Publishing to Anthropic's Directory (`claude.ai`)

To submit this plugin to the official Anthropic Claude Directory:

1. **Pre-submission Checklist**:
   - Ensure the repository `https://github.com/sahalhes/waymesh` is public.
   - Verify `icon` (`./assets/icon.png`) and legal URLs are present in `.claude-plugin/plugin.json`.
   - Validate metadata and remote MCP health.
2. **Submit via Developer Portal**:
   - Navigate to [claude.ai/directory/manage](https://claude.ai/directory/manage).
   - Click **Submit a Plugin**.
   - Provide the GitHub repository URL (`https://github.com/sahalhes/waymesh`) and directory path (`connectors/claude/waymesh-plugin`).
3. **Distribution**:
   Once approved by Anthropic, users can add Waymesh directly on **claude.ai** and inside **Cowork**, automatically syncing to their Claude Code sessions as `waymesh-mobility@synced`.
