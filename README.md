# zed-cargo-appraiser

A [Zed](https://zed.dev) extension for the [cargo-appraiser](https://github.com/washanhanzi/cargo-appraiser) LSP server.

## Features

- **Version Decorations** - Shows installed vs latest versions inline
- **Hover Information** - Available versions, features, and git references  
- **Code Actions** - Update dependencies
- **Audit Warnings** - Security vulnerability information

> **Note**: Enable Zed's inlay hints to see version decorations.

## Installation

Install from the Zed Extensions panel.

## Configuration

Configure in your Zed `settings.json`:

```jsonc
{
  "lsp": {
    "cargo-appraiser": {
      // Pin to a specific version (optional)
      "settings": {
        "version": "0.3.0"
      },
      // LSP initialization options
      "initialization_options": {
        // See upstream docs for all options
      }
    }
  }
}
```

### Custom Binary

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

## Documentation

For full configuration options (decoration formatters, audit settings, etc.), see the [cargo-appraiser documentation](https://github.com/washanhanzi/cargo-appraiser#config).

## Troubleshooting

### macOS Gatekeeper

If macOS blocks the binary:

```bash
xattr -d com.apple.quarantine ~/.zed/extensions/*/cargo-appraiser-*/cargo-appraiser
```

### Audit not working

Install cargo-audit:

```bash
cargo install cargo-audit --locked
```
