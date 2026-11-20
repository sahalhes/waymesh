# Waymesh Claude Integration

This directory contains the official **Claude Code Plugin**, **Marketplace Manifest**, and **Claude Desktop MCP Configuration** for the Waymesh decentralized mobility and logistics protocol.

---

## Directory Overview

```text
connectors/claude/
├── .claude-plugin/
│   └── marketplace.json            # Local marketplace manifest
├── waymesh-plugin/
│   ├── .claude-plugin/
│   │   └── plugin.json             # Official Claude Code plugin manifest
│   ├── .mcp.json                   # Remote streamable HTTP MCP server config
│   ├── assets/
│   │   ├── icon.png                # Plugin square icon
│   │   └── logo.png                # Plugin brand logo
│   ├── skills/
│   │   └── waymesh-rideshare/
│   │       └── SKILL.md            # Agent skill for rides, safe carpooling & logistics
│   └── README.md                   # Plugin specific guide
├── waymesh-plugin.zip              # Pre-packaged distributable zip archive
└── README.md                       # This guide
```

---

## 1. Quick Testing (Local Run without Installation)

You can launch Claude Code with the plugin loaded for a single session without altering global configuration:

```bash
claude --plugin-dir connectors/claude/waymesh-plugin
```

Test commands inside the Claude Code session:
```text
/waymesh-mobility:waymesh-rideshare
```
Or ask directly:
> *"Find me safe carpooling from CET to Kochuveli Railway Station for 5:00 PM today. Passenger is female."*

---

## 2. Install via Marketplace

### Option A: From GitHub (Team / Worldwide)
Once pushed to GitHub, anyone can add the repository directly as a Claude marketplace:

```bash
# Add Waymesh repository as marketplace
claude plugin marketplace add sahalhes/waymesh

# Install Waymesh Mobility plugin
claude plugin install waymesh-mobility
```

### Option B: From Local Repository
```bash
# Add local directory as marketplace
claude plugin marketplace add ./connectors/claude

# Install Waymesh Mobility plugin
claude plugin install waymesh-mobility
```

---

## 3. Claude Desktop Configuration

To use Waymesh with Claude Desktop, add the streamable HTTP MCP server to your Claude Desktop config:

**Config Path:**
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux:** `~/.config/Claude/claude_desktop_config.json`

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

---

## 4. Submitting to Anthropic's Official Directory

To make Waymesh discoverable on [claude.ai/directory](https://claude.ai/directory) and inside **Cowork**:

1. Ensure the repo `https://github.com/sahalhes/waymesh` is public and pushed to main.
2. Sign in to the Anthropic Developer Portal at [claude.ai/directory/manage](https://claude.ai/directory/manage).
3. Click **Submit a Plugin**.
4. Enter:
   - **Repository:** `https://github.com/sahalhes/waymesh`
   - **Plugin Directory:** `connectors/claude/waymesh-plugin`
   - **Manifest:** Verified automatically from `.claude-plugin/plugin.json`
   - **MCP Server:** Verified automatically from `https://waymesh.icvalley.tech/mcp`
5. Once approved, the plugin is installable from `claude.ai` and automatically syncs across user sessions as `waymesh-mobility@synced`.

---

## 5. Building the Distributable Zip

To regenerate the standalone zip archive:

```bash
cd connectors/claude/waymesh-plugin
zip -r ../waymesh-plugin.zip . -x ".*" -x "__MACOSX"
cp ../waymesh-plugin.zip ../../../waymesh-claude-plugin.zip
```
