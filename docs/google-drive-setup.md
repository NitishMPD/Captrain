# Google Drive backup setup
Author: Nitish

Captrain uploads directly to the connected user's Google Drive. There is no Captrain storage server or separate Captrain account. Google storage limits and API quotas apply; there is no unlimited free storage. The same Google OAuth application must be used on every PC that accesses these backups.

## Register the desktop application

1. Open [Google Cloud Console](https://console.cloud.google.com/) and create or select a project.
2. Enable **Google Drive API** under APIs & Services → Library.
3. Configure **Google Auth Platform** branding, audience and contact details. For a personal trial, choose an external testing app and add your own Google account as a test user. For public distribution, complete Google's production publishing and applicable verification requirements. A testing app's refresh token generally expires after seven days with Drive permissions.
4. The requested scopes are `https://www.googleapis.com/auth/drive.appdata`, `openid` and `email`. Captrain needs its own app storage and the connected account's email; it does not request access to the rest of Drive.
5. Under Clients, create an OAuth client of type **Desktop app**. Do not use a Web application client or a service account. Download the JSON.
6. Save the downloaded file locally as `cloud/oauth-client.json` (gitignored), or keep it in another private location. The committed `cloud/oauth-client.example.json` shows the expected shape; its example values cannot sign in. Keep the actual downloaded JSON intact.

Google Cloud menu labels can change. Official guides: [Desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [OAuth app setup](https://developers.google.com/workspace/guides/configure-oauth-consent), [Drive app data](https://developers.google.com/workspace/drive/api/guides/appdata).

## Connect Captrain

1. Open Settings → Cloud backup → **Developer setup** → **Import OAuth file…** and select the downloaded Desktop app JSON.
2. Choose Screenshots, Web links and optionally Videos. Enable **Include existing library items in backup** if you want history uploaded; otherwise only items created after connecting are eligible. Videos start disabled.
3. Choose **Connect Google Drive**, sign in in the system browser and grant permission. Captrain uses a random loopback port, checks OAuth state and exchanges the code with PKCE. Return to Settings to see the connected email or any sign-in error.
4. Backups check for changes every minute while Captrain is running. **Back up now** performs a fresh scan. Offline/network failures leave local files intact and retry on a later check. Pause prevents automatic and manual backup; restore remains available.

Imported client configuration and refresh tokens are stored as Windows-user-encrypted `cloud-auth.bin` in `%LOCALAPPDATA%\com.nitish.captrain\`. Preferences, backup cutoff and last success time are in `cloud-settings.json` beside it. Tokens never enter React or the repository. Protect the downloaded OAuth JSON and do not paste its secret into chat. DPAPI credentials cannot be copied to another Windows account or PC; import the same Desktop OAuth JSON and sign in there instead.

To switch Google accounts, disconnect first; Reconnect renews the current account without changing its backup cutoff. Disconnect clears local account credentials, keeps the imported OAuth client and preserves both local and remote data. To revoke the app's grant, remove its access in [Google Account connections](https://myaccount.google.com/connections). Google may also remove app data when its connection/data is removed; do not rely on cloud storage as your only copy.

## Restore and retention

On the second PC, import the same OAuth app JSON, connect the same Google account, select the item types, then choose **Restore missing items**. The Include existing setting controls uploads only and does not limit restore. Existing item IDs are kept unchanged, including local edits. Organizational folder names and memberships accompany backed-up items. PC folder memberships, export paths, Windows folder links, device settings, credentials and thumbnails are not uploaded; thumbnails are regenerated after download. Empty organizational folders have no standalone backup record.

Cloud copies remain after local deletion. Restoring can therefore bring a locally deleted item back. This release is backup and additive restore, not automatic two-way library synchronization or a cloud deletion manager. Remote files are hidden in the application's Drive app-data folder and are not ordinary browsable My Drive files. They are protected by Google account permissions, not end-to-end encryption.

Each uploaded item includes versioned metadata and a SHA-256 checksum. Downloads are staged outside the library lock, checked for size and checksum, and validated before atomic library commit. Screenshots must decode as PNG with matching dimensions and a maximum 256 MiB decode allocation. Shortcut bytes must match the validated HTTP/HTTPS URL. MP4s retain their bytes; playable media is not guaranteed by a checksum. Existing/unindexed local files are never overwritten. Completed restores remain if a later item fails; the error is visible and Restore can be retried.

Uploads use bounded 8 MiB resumable chunks and recover confirmed offsets after an interrupted request. Sessions are not persisted across app exit: unfinished files restart on the next backup, while completed files are deduplicated. Google access-token expiry during a long transfer also requires a later retry. Cancellation stops between items/current transfers rather than interrupting a library transaction. Crashed restore staging directories named `cloud-restore-<uuid>` may remain in local app data; they are not imported or deleted automatically. Preserve uncertain files before cleaning them up.

Damaged cloud settings/credentials block cloud operations only. Exit Captrain, preserve the damaged `cloud-settings.json` or `cloud-auth.bin` elsewhere, remove that file and reopen Settings. Restore preferences or reimport the OAuth JSON and reconnect as appropriate. Local captures and `index.json` remain independent.

## Verification boundary

Type checking and focused native tests cover storage rollback, additive restore, OAuth callback validation, Windows credential encryption, metadata filtering and interrupted-upload resume against a local HTTP fixture. Real Google consent, quotas, account revocation and cross-PC restore require testing with a registered OAuth app. No app launch or release build is part of these source checks.
