# Tauri AutoFlow OAuth Server Design

## Goal

Expose a Codex2API-compatible local HTTP integration service from the Tauri app so AutoFlow can add newly registered OpenAI/Codex accounts by calling:

- `POST /api/admin/oauth/generate-auth-url`
- `POST /api/admin/oauth/exchange-code`

The service is disabled by default. The user enables it explicitly from the app UI.

## Confirmed Decisions

- The feature belongs to the Tauri backend in `src-tauri`, not AutoFlow and not CLIProxyAPI.
- Development must not happen on `main`; use a `codex/` feature branch.
- The service starts only after the user clicks an enable/start control.
- The listening port is user-configurable.
- The admin key is generated automatically the first time the service is enabled, persisted in app settings, and exposed in the UI for copying into AutoFlow.
- AutoFlow supplies the generated key via `X-Admin-Key`.

## Existing System

The app already has the core OAuth pieces in `src-tauri/src/oauth.rs`:

- OpenAI auth URL constants and client ID.
- PKCE verifier/challenge generation.
- Auth URL construction.
- Code exchange against `https://auth.openai.com/oauth/token`.
- Token response parsing and error formatting.
- Account saving through `save_account_record`.

The app already has account persistence in `src-tauri/src/accounts.rs`:

- `auth_json_from_token_response` converts an OAuth token response into the app's wrapped auth format.
- `summary_from_auth_json` builds account metadata from `auth.json` and ID token claims.
- `save_account_record` writes `metadata.json`, `auth.json`, and `original.json` under the app store account directory.

The app already has persistent settings in `src-tauri/src/settings.rs`:

- `Settings` is saved as JSON under the app store directory.
- `oauth_callback_port` is currently the callback port used by the app's own interactive login flow.

## Proposed Architecture

Add a dedicated local integration service module, for example `src-tauri/src/autoflow_oauth_server.rs`.

This module owns:

- The local HTTP listener lifecycle.
- The in-memory OAuth session table.
- The Codex2API-compatible request and response shapes.
- Admin key validation using the persisted app setting.

The existing `oauth.rs` should expose or refactor reusable helpers rather than duplicate OAuth logic:

- PKCE generation.
- Auth URL construction.
- Code exchange.
- Flat/original account JSON construction, or an equivalent save helper.

The existing `accounts.rs` should remain the account persistence authority.

## Settings Model

Extend `Settings` with fields similar to:

- `autoflow_oauth_server_enabled: bool`
- `autoflow_oauth_server_port: u16`
- `autoflow_oauth_admin_key: String`

Defaults:

- Server disabled.
- Port `8080`.
- Empty admin key until first enable/start.

When the user enables or starts the service and `autoflow_oauth_admin_key` is empty, generate a high-entropy URL-safe random key and persist it. Existing keys are reused until the user explicitly resets them.

## UI Surface

Add an AutoFlow integration section to the settings view.

Expected controls:

- Port input.
- Start service button.
- Stop service button.
- Current service URL display, for example `http://127.0.0.1:8080/admin/accounts`.
- Admin key masked display.
- Copy admin key button.
- Reset admin key button.
- Service status text.

The URL shown to the user should match the value AutoFlow expects in its Codex2API address field. AutoFlow derives the origin, so displaying `/admin/accounts` is enough even if the Tauri app does not serve that page.

## Endpoint Design

### `POST /api/admin/oauth/generate-auth-url`

Request:

- Header `X-Admin-Key: <admin key>`
- JSON body may be `{}` or empty.

Behavior:

1. Validate the admin key.
2. Generate `session_id`.
3. Generate `state`.
4. Generate PKCE `code_verifier` and `code_challenge`.
5. Build the OpenAI auth URL with redirect URI `http://localhost:{settings.oauth_callback_port}/auth/callback`.
6. Store an in-memory session with `session_id`, `state`, `code_verifier`, `redirect_uri`, `created_at`, `expires_at`, and `used_at`.
7. Return `auth_url` and `session_id`.

Response:

```json
{
  "auth_url": "https://auth.openai.com/oauth/authorize?...",
  "session_id": "sess_..."
}
```

### `POST /api/admin/oauth/exchange-code`

Request:

- Header `X-Admin-Key: <admin key>`
- JSON body:

```json
{
  "session_id": "sess_...",
  "code": "callback-code",
  "state": "oauth-state"
}
```

Behavior:

1. Validate the admin key.
2. Validate required fields.
3. Find the session by `session_id`.
4. Reject missing, expired, or already used sessions.
5. Compare request `state` with session `state`.
6. Exchange `code` using the session `redirect_uri` and `code_verifier`.
7. Save the account using the existing account persistence path.
8. Mark the session used.
9. Return a success message plus account id and email if known.

Response:

```json
{
  "message": "OAuth account user@example.com added successfully",
  "id": "account-id",
  "email": "user@example.com"
}
```

## Error Contract

AutoFlow reads `message`, `error`, `detail`, or `reason`. The Tauri service should consistently return:

```json
{
  "message": "specific failure reason"
}
```

Recommended statuses:

- `401` for missing or invalid `X-Admin-Key`.
- `400` for invalid JSON, missing fields, state mismatch, expired session, or used session.
- `500` for persistence or unexpected token exchange plumbing failures.

Token values must never be included in logs or error responses.

## Security

- Bind only to `127.0.0.1`.
- Require `X-Admin-Key` on both endpoints.
- Generate the admin key locally; never read Codex app credentials as an admin key.
- Keep OAuth sessions in memory only.
- Expire sessions after 10 minutes.
- Mark sessions as used after successful account persistence. A repeated `exchange-code` call for the same session must return a clear "OAuth session already used" error and must not save a duplicate account.
- Do not log full access tokens, refresh tokens, or ID tokens.

## Testing Strategy

Unit tests:

- Admin key generation returns non-empty URL-safe strings.
- Auth URL responses contain `state` and a session id.
- Exchange rejects missing, expired, used, and state-mismatched sessions.
- Exchange calls token-to-account persistence with the expected account summary.

Integration-style tests:

- Start the local server on an ephemeral port.
- Call `generate-auth-url` with missing and valid keys.
- Call `exchange-code` with a stubbed token exchanger so tests do not hit OpenAI.
- Verify saved account metadata and response shape.

Frontend tests or focused build checks:

- Settings model serialization remains backward-compatible for existing settings.
- Settings UI can start/stop service and copy/reset the admin key.

## Open Implementation Notes

- The current dependency set has `std::net::TcpListener` and `reqwest` but no HTTP server framework. The implementation can either use a small manual HTTP parser for two local JSON POST endpoints or add a lightweight HTTP server crate. Prefer the smallest maintainable option during planning.
- The existing interactive OAuth login uses a global single pending session. The AutoFlow service should use its own session map so external registration flows do not interfere with manual login.
- The existing OAuth callback port setting should remain separate from the AutoFlow service port. The callback port is embedded in the OpenAI auth URL, while the service port is where AutoFlow calls the Tauri app.
