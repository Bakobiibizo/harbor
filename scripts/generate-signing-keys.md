# Harbor Release Signing Keys Setup

The Tauri updater requires signed releases to ensure authenticity. Follow these steps to set up signing.

## 1. Generate Signing Keys

Run this command to generate a new keypair:

```bash
# Install tauri-cli if not already installed
cargo install tauri-cli

# Generate keys (will prompt for a password)
cargo tauri signer generate -w ~/.tauri/harbor.key
```

This creates:
- `~/.tauri/harbor.key` - Your **private key** (keep this secret!)
- Outputs the **public key** to the console

## 2. Update tauri.conf.json

Copy the public key from the output and replace `UPDATER_PUBKEY_PLACEHOLDER` in `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk...",
      "endpoints": [
        "https://github.com/nicholasoxford/harbor/releases/latest/download/latest.json"
      ]
    }
  }
}
```

## 3. Add Secrets to GitHub

Go to your repository Settings → Secrets and variables → Actions, then add:

1. **TAURI_SIGNING_PRIVATE_KEY**
   - Value: The contents of `~/.tauri/harbor.key`
   - Run: `cat ~/.tauri/harbor.key` to get it

2. **TAURI_SIGNING_PRIVATE_KEY_PASSWORD**
   - Value: The password you used when generating the key

## 4. Creating a Release

### Option A: Tag-based release (recommended)

```bash
# Update version in package.json and src-tauri/tauri.conf.json
# Then create and push a tag:
git tag v0.2.0
git push origin v0.2.0
```

The GitHub Action will automatically:
1. Create a draft release
2. Build for Windows, macOS (Intel + ARM), and Linux
3. Sign all artifacts
4. Upload them to the release
5. Generate the `latest.json` file for the updater
6. Publish the release

### Option B: Manual trigger

Go to Actions → Release → Run workflow, and enter the version number.

## 5. Verifying Updates Work

After a release is published:

1. Open Harbor on a device with an older version
2. Go to Settings → Updates
3. Click "Check for Updates"
4. If an update is available, click "Install Update"

## Troubleshooting

### "Invalid signature" error
- Make sure the public key in `tauri.conf.json` matches your private key
- Ensure the private key in GitHub secrets is complete (including BEGIN/END lines)

### Updates not detected
- Check that `latest.json` exists at the endpoint URL
- Verify the version in `latest.json` is higher than the installed version

### Build failures
- Check GitHub Actions logs for specific errors
- Ensure all secrets are properly configured
