# Donna Usage

Donna is a local-first personal assistant shell.

## Start

Run the desktop UI:

```sh
donna
```

Run the auth setup placeholder:

```sh
donna --auth
```

## Config

Donna creates non-secret TOML settings at `~/.config/donna/donna.toml` on Linux.
Secrets belong in OS secret storage and are referenced by name from TOML.

The initial config includes UI colors, the `donna` avatar, chat models, a background
task model placeholder, Microsoft metadata placeholders, notes metadata, task folder
settings, memory policy, local database path, stale-data policy, offline behavior,
and attention settings.

## Local Data

Donna creates its local SQLite database at the configured `[data].database_path`.
On Linux the default is under `~/.local/share/donna/`. The database stores
structured memories, todos, people, follow-ups, task findings, synced Microsoft
data, sync state, notes metadata, local offline state, and audit records.

Donna does not create a table for raw local chat transcripts. Chat can produce
structured memories or todos later, but the transcript itself remains in memory.

## Chat

Chat messages are held in memory for the current session only. Donna does not write
raw chat transcripts to the config file or a local database.

Supported shell commands:

- `/exit` requests app exit.
- `/hide` requests that Donna minimize to the desktop. Some window managers,
  especially Wayland compositors, may ignore app-driven minimize requests; Donna
  shows a short fallback note and keeps running.

Press Tab to cycle the selected chat model. The selected model is saved to the
TOML config and applies to the next message.

## Avatars

Donna embeds avatar images in the binary. The configured character defaults to
`donna`; missing characters or state images fall back to Donna's default image.

## Developer Verification

Use the same local guardrails as CI before handing work to another agent:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

On Debian or Ubuntu, the Linux desktop build expects `pkg-config`, Mesa GL,
Wayland, X11, XCB, Xi, XRandR, and xkbcommon development packages. See
`README.md` for the exact package list used by CI.
