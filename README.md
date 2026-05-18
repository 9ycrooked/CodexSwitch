# Codex Switch

Codex Switch is a Windows desktop app for managing multiple local Codex OAuth accounts and switching the active Codex login state safely.

It is built with Tauri, Vue 3, TypeScript, and Yarn.

## Features

- Import one or many Codex OAuth credential files.
- Store multiple accounts locally in an app-managed account library.
- Switch the active Codex account by replacing `auth.json` and merging `config.toml`.
- Preserve existing local Codex tools, plugins, MCP servers, projects, and other configuration where possible.
- Back up the current Codex state before switching accounts.
- Restore previous backups.
- Log in with OAuth using an isolated WebView2 profile per account.
- Monitor Codex usage status from the in-app quota page.
- Refresh account tokens manually.
- Use a compact dark desktop UI with a custom title bar.

## Important Security Notice

Codex Switch stores imported OAuth credentials locally. These files can contain refresh tokens.

Do not share the app data directory, account library, backups, or exported credential files with anyone.

This app is intended for managing your own accounts on your own Windows user profile. It does not implement fingerprint spoofing, anti-detection behavior, or platform risk-control bypassing.

## What Codex OAuth Files Are For

Codex OAuth JSON files are local login credentials, not regular OpenAI API keys.

In the current Codex desktop and CLI setup, both can use the same Codex home directory, such as:

```text
C:\Users\Y\.codex
```

Replacing `auth.json` in that directory can affect both Codex Desktop and Codex CLI login state for the same Windows user.

## Default Paths

Codex Switch defaults to:

```text
C:\Users\Y\.codex
```

The app manages imported accounts and backups in its own application data directory.

## Development

Install dependencies:

```powershell
yarn install
```

Run the frontend only:

```powershell
yarn dev
```

Run the Tauri desktop app:

```powershell
yarn tauri dev
```

Build the frontend:

```powershell
yarn build
```

Build the desktop app:

```powershell
yarn tauri build
```

## Project Structure

```text
src/                  Vue application
src-tauri/            Tauri Rust backend
src-tauri/src/lib.rs  Account switching, OAuth, backup, and quota logic
DESIGN.md             UI design notes
```

## Notes

- Quota monitoring is best effort because Codex usage endpoints are not guaranteed to be stable public APIs.
- Quota checks are manual and are not polled in the background.
- OAuth login uses normal browser/WebView behavior with isolated profiles. It does not spoof Canvas, WebGL, hardware, fonts, or device fingerprints.

## Release Workflow

Codex Switch uses Tauri updater artifacts and GitHub Draft Releases.

1. Update the version in:
   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`

2. Commit and push:

   ```powershell
   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
   git commit -m "chore: release v0.1.1"
   git push
   ```

3. Edit `release/update-policy.json` if this version should change updater behavior:

   ```json
   {
     "check_updates_on_startup": true,
     "force_update_on_startup": false,
     "message": "发现新版本时会显示更新内容，你可以选择立即更新或稍后处理。"
   }
   ```

   `check_updates_on_startup` controls whether the app checks on launch. `force_update_on_startup` controls whether a discovered update can be dismissed. The default policy checks on startup and does not force the update.

4. Create and push a tag:

   ```powershell
   git tag v0.1.1
   git push origin v0.1.1
   ```

5. Wait for GitHub Actions to create a Draft Release.

6. Open the Draft Release and edit the Release body. This body is displayed in the app update dialog.

7. Confirm the Windows installer, updater artifacts, `latest.json`, and `update-policy.json` are attached.

8. Publish the Release.

After the Release is published, Codex Switch can detect it on startup and show the update dialog.

## License

MIT
