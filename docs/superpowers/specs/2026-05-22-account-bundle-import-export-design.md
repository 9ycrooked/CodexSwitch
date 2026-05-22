# Account Bundle Import Export Design

## Goal

Codex Switch will export and import selected accounts as one ZIP bundle that binds each account credential to its isolated login environment.

## Scope

- Export selected accounts.
- Include each selected account's `auth.json`.
- Include the account's saved isolated browser profile directory when it exists.
- Import only Codex Switch account bundles.
- Stop supporting direct `.json` / `.toml` batch import.
- Keep the local account store layout unchanged.

## Bundle Format

The export file is a ZIP with this shape:

```text
manifest.json
accounts/<account-id>/auth.json
profiles/<account-id>/...      # optional
```

`manifest.json` contains:

- `format`: `codex-switch.account-bundle`
- `version`: `1`
- `exported_at`: RFC3339 timestamp
- `accounts`: selected account entries

Each account entry includes the display metadata, the auth path, the SHA-256 of `auth.json`, and whether a profile was included.

## Import Validation

Import accepts only `.zip` files. The backend validates:

- The ZIP opens successfully.
- The root `manifest.json` exists and has the expected format/version.
- Each account entry points to `accounts/<id>/auth.json`.
- ZIP entry names are relative, normalized, and do not contain `..`, drive prefixes, absolute paths, or backslashes.
- Each `auth.json` SHA-256 matches the manifest.
- Each `auth.json` normalizes to the existing Codex auth shape.

Invalid bundle structure rejects the import. A damaged individual account is reported and skipped when the rest of the bundle is valid.

## Local Storage

Imported accounts continue to use the existing account store:

```text
accounts/<account-id>/metadata.json
accounts/<account-id>/auth.json
accounts/<account-id>/original.json
```

No `config.toml`, quota state, usage state, backups, or global browser data are imported. If a profile is included, it is extracted to the configured `browser_profile_dir/<account-id>` and `metadata.json.browser_profile_dir` points there.

## Relogin Behavior

The app will expose a per-account relogin action. It starts OAuth using that account id as the profile id, so the existing imported login environment is reused. If the OAuth result belongs to a different account identity, a follow-up conflict flow can be added later; the first implementation preserves the current save behavior and keeps the same profile binding.

## UI

- The account page top bar adds an export button.
- Export opens a multi-select modal with account rows and profile availability.
- Batch import chooses only ZIP bundles and supports dropping ZIP files onto the account page.
- Old `.json` / `.toml` selections show a clear unsupported-format message.
- Account cards show a relogin action that reuses the account profile.

## Security Notes

Bundles include `refresh_token` and may include cookies, local storage, and session data. The UI labels them as sensitive login bundles and warns users not to share them.
