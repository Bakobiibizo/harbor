# Unsigned Windows beta

Harbor's private Windows beta is not Authenticode-signed. Windows SmartScreen may show **Windows protected your PC** when the installer is downloaded from a browser.

Only install a beta artifact obtained directly from the official Harbor GitHub release. In the SmartScreen dialog, select **More info**, verify the application is **Harbor**, and select **Run anyway**.

The absence of Authenticode means Windows cannot verify the publisher identity. It does not disable Harbor's separate Tauri updater signature: updater artifacts are signed with Harbor's existing private updater key and verified against the public key embedded in the application.

For each beta release, publish SHA-256 checksums alongside the installer so testers can verify the downloaded file before running it:

```powershell
Get-FileHash .\Harbor_*.msi -Algorithm SHA256
Get-FileHash .\Harbor_*-setup.exe -Algorithm SHA256
```
