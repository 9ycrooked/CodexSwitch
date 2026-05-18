# Defensive Tauri Commands

This project should treat every Tauri command as part of the UI responsiveness
contract. Before adding or editing a command, classify the work first and choose
the least blocking implementation that still keeps the behavior simple.

## Command Classes

### Fast Commands

Use a normal synchronous command only when the work is small, CPU-light, and
does not wait on external systems.

Examples:

- Reading already-loaded state from memory
- Returning small settings values
- Formatting simple response objects

### Blocking Commands

Use an `async fn` command with `tauri::async_runtime::spawn_blocking` when the
work may block the command thread.

Examples:

- File reads or writes
- JSON or TOML parsing for account/config files
- Backup and restore operations
- Running child processes
- Blocking HTTP requests
- Token refresh and quota checks

Implementation rule:

```rust
#[tauri::command]
pub async fn example_command(app: AppHandle) -> AppResult<ExampleResult> {
    run_blocking(move || example_command_inner(&app)).await
}
```

Keep the real work in an inner synchronous function when that keeps the code
clear and testable.

### Waiting Commands

Use bounded waits and status-oriented behavior for commands that wait for an
external condition.

Examples:

- Closing Codex before switching accounts
- Waiting for an OAuth callback
- Polling for a process to exit

Implementation rules:

- Always set a timeout.
- Prefer polling with short intervals over a fixed long sleep.
- Return enough status for the frontend to tell the user what happened.
- Avoid unbounded loops.

### Long-Running Commands

For operations that may run long enough to feel like a task, keep them explicit
and user-driven.

Examples:

- Batch account import
- Full backup restore
- Network quota checks

Implementation rules:

- Run off the command thread.
- Keep the UI interactive where possible.
- Surface per-operation loading state instead of locking the whole app.
- Make partial failure messages actionable.

## Windows Process Rule

Any Windows child process launched by the app must avoid opening a visible
console window unless the feature explicitly needs an interactive terminal.

Use the shared helper in `src-tauri/src/process.rs` for process commands such as:

- `taskkill`
- `tasklist`
- Browser fallback launchers
- `rundll32`

Do not call `std::process::Command::new(...)` directly for Windows helper
processes without checking whether the command can flash a console.

## Frontend Loading Rule

Avoid one global `busy` flag for unrelated operations. Use operation-specific
loading state so a slow quota check does not block account switching, backups,
settings edits, or navigation.

Recommended loading scopes:

- `import`
- `switch:<account_id>`
- `refresh:<account_id>`
- `quota:<account_id>`
- `backup`
- `restore:<backup_id>`
- `settings`

Button disabled states should match the specific operation they trigger.

## Defensive Checklist

Before merging a command change, check:

- Could this touch disk, network, a process, or a timer?
- Could this take more than a few milliseconds on a slow machine?
- Could this open a console window on Windows?
- Does failure leave auth/config files in a valid state?
- Does the UI show the user which operation is running?
- Does the command have a bounded timeout if it waits?

If the answer is uncertain, treat the command as blocking and move it off the
command thread.
