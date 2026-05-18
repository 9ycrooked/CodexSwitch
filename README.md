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

## License

MIT
