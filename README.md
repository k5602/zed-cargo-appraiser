# zed-cargo-appraiser

A [Zed](https://zed.dev) extension for the [cargo-appraiser](https://github.com/washanhanzi/cargo-appraiser) LSP server.

## Features

Cargo Appraiser provides quality-of-life improvements for `Cargo.toml` files:

- **Version Decorations** - Shows installed vs latest versions inline
- **Hover Information** - Available versions, features, and git references
- **Code Actions** - Update individual dependencies or entire workspace
- **Audit Warnings** - Security vulnerability information (requires `cargo-audit`)
- **Workspace Navigation** - Go to definition for workspace members

> **Note**: Make sure to enable Zed's inlay hints to see version decorations.

## Installation

Install from the Zed Extensions panel, or add to your `extensions.toml`:

```toml
[cargo-appraiser]
```

## Configuration

Configure in your Zed `settings.json` under `lsp.cargo-appraiser`:

### Basic Configuration

```jsonc
{
  "lsp": {
    "cargo-appraiser": {
      // Pin to a specific version (optional)
      "settings": {
        "version": "0.3.0"
      }
    }
  }
}
```

### Custom Binary Path

Use a locally built or custom binary:

```jsonc
{
  "lsp": {
    "cargo-appraiser": {
      "binary": {
        "path": "/path/to/cargo-appraiser",
        "arguments": ["--renderer", "inlayHint"]
      }
    }
  }
}
```

### LSP Initialization Options

Customize decorations and audit settings:

```jsonc
{
  "lsp": {
    "cargo-appraiser": {
      "initialization_options": {
        "decorationFormatter": {
          "latest": "✅ {{installed}}",
          "local": "Local",
          "not_resolved": "Not Resolved",
          "waiting": "Waiting...",
          "mixed_upgradeable": "🚀🔒 {{installed}} -> {{latest_matched}}, {{latest}}",
          "compatible_latest": "🚀 {{installed}} -> {{latest}}",
          "noncompatible_latest": "🔒 {{installed}}, {{latest}}",
          "yanked": "❌ yanked {{installed}}, {{latest_matched}}",
          "git": "🐙 {{commit}}"
        },
        "audit": {
          "disabled": false,
          "level": "warning"
        }
      }
    }
  }
}
```

### Decoration Formatter Options

| Field | Description | Template Variables |
|-------|-------------|-------------------|
| `latest` | Dependency is at latest version | `{{installed}}` |
| `local` | Local path dependency | - |
| `not_resolved` | Not resolved (platform mismatch) | - |
| `waiting` | Waiting for cargo to resolve | - |
| `mixed_upgradeable` | Compatible upgrade available, latest incompatible | `{{installed}}`, `{{latest_matched}}`, `{{latest}}` |
| `compatible_latest` | Can update to latest | `{{installed}}`, `{{latest}}` |
| `noncompatible_latest` | Latest is incompatible | `{{installed}}`, `{{latest}}` |
| `yanked` | Current version is yanked | `{{installed}}`, `{{latest_matched}}` |
| `git` | Git dependency | `{{ref}}`, `{{commit}}` |

### Audit Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disabled` | `bool` | `false` | Disable vulnerability scanning |
| `level` | `string` | `"warning"` | `"warning"` or `"vulnerability"` |

> **Note**: Audit requires `cargo-audit` to be installed: `cargo install cargo-audit --locked`

## Troubleshooting

### Binary not downloading

If the extension fails to download the binary, check:

1. **Network connectivity** - Ensure you can reach GitHub
2. **Platform support** - Binaries are available for:
   - macOS (arm64, amd64)
   - Linux (arm64, amd64)
   - Windows (amd64)

### Audit warnings not showing

Make sure `cargo-audit` is installed and in your PATH:

```bash
cargo install cargo-audit --locked
```

### macOS Gatekeeper

If macOS blocks the binary, you may need to allow it:

```bash
xattr -d com.apple.quarantine /path/to/cargo-appraiser
```

## Links

- [cargo-appraiser LSP](https://github.com/washanhanzi/cargo-appraiser) - Upstream project
- [VS Code Extension](https://marketplace.visualstudio.com/items?itemName=washan.cargo-appraiser)
- [Neovim Plugin](https://github.com/washanhanzi/cargo-appraiser.nvim)
