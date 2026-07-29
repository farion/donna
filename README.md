# Donna

Donna is a single-user, local-first personal work-life assistant built with Rust
and egui.

## User Guide

See [`docs/usage.md`](docs/usage.md) for MVP setup, configuration, Microsoft
Graph auth, privacy boundaries, task files, Obsidian notes behavior, approval
gates, and troubleshooting.

Run `donna --help` for CLI options (`--auth`, `--wakeup`, `--reset-sync`,
`--help`); see [Command-Line Options](docs/usage.md#command-line-options) for
details.

## Tools

Donna gives the LLM a set of local tools it can invoke during chat by emitting
a JSON call (e.g. `{"tool":"create_todo","arguments":{"title":"..."}}`). Each
tool is implemented in its own file under
[`crates/donna-harness/src/tools/`](crates/donna-harness/src/tools/), with its
model-facing description in a matching Markdown file under
[`assets/tools/`](assets/tools/). Every call is logged (tool name, arguments,
and result) via `eprintln!` in `execute_model_tool_call`
(`crates/donna-harness/src/tools/mod.rs`).

This list must be kept in sync whenever a tool is added, removed, or changed
(see `AGENTS.md`).

The system prompt also injects the real current date and time (see
`assets/prompts/context/current_date_time.md`) so the model computes
date-based tool arguments from actual "now" instead of guessing or copying an
example timestamp from a tool description.

| Tool | Arguments | Description |
| --- | --- | --- |
| `list_open_todos` | none | Lists up to 20 open todos. |
| `list_completed_todos` | none | Lists up to 20 completed todos. |
| `create_todo` | `title` (required), `severity` (low/middle/high, default middle), `notes`, `related_topic`, `due_at` | Creates a new todo. |
| `complete_todo` | `todo_id`/`id` | Marks a todo done. |
| `delete_todo` | `todo_id`/`id` | Deletes a todo. |
| `update_todo_severity` | `todo_id`, `severity`/`priority` | Changes a todo's severity. |
| `update_todo_due_at` | `todo_id`, `due_at` (or `null` to clear) | Changes or clears a todo's due date. |
| `calendar_list_appointments` | `date_from`/`from`, `date_to`/`to`, `limit` (default 25) | Lists calendar events in a specific date range other than today/tomorrow. |
| `calendar_search_appointments` | `text`/`query`/`title`, `persons`/`people`/`organizer`, date range, `limit` | Searches calendar events. |
| `list_today_appointments` | none | Lists today's calendar events (computed from the real clock, no date args needed). |
| `list_tomorrow_appointments` | none | Lists tomorrow's calendar events (computed from the real clock, no date args needed). |
| `next_appointment` | none | Finds the single soonest upcoming calendar event (computed from the real clock, no date args needed). |
| `calendar_create_appointment` | `subject`, `starts_at`, `ends_at`, ... | **Stub** — requires explicit approval, not yet wired to Graph. |
| `calendar_delete_appointment` | `appointment_id` | **Stub** — requires explicit approval, not yet wired to Graph. |
| `calendar_move_appointment` | `appointment_id`, ... | **Stub** — requires explicit approval, not yet wired to Graph. |
| `outlook_list_mails` | `date_from`/`from`, `date_to`/`to`, `limit` (default 20) | Lists synced Outlook mail. |
| `outlook_search_mails` | `title`/`subject`, `text`/`query`, `person`/`sender`, date range, `limit` (default 20) | Searches synced Outlook mail. |
| `outlook_send_mail` | `to`, `subject`, `body` | **Stub** — requires explicit approval, not yet wired to send. |
| `teams_list_chat_messages` | date range, `limit` (default 25) | Lists 1:1/group chat messages. |
| `teams_list_channel_messages` | date range, `limit` (default 25) | Lists channel messages. |
| `teams_list_chats` | `limit` (default 50) | Lists chats (non-channel conversations). |
| `teams_list_channels` | `limit` (default 50) | Lists channel conversations. |
| `teams_search_messages` | `text`/`query`, `person`/`sender`, date range, `limit` (default 25) | Searches Teams messages. |
| `summarize_teams_conversation` | `person`/`name`/`contact` (required) | Summarizes recent messages from a person. |
| `teams_send_message` | `chat_id`/`channel_id`/`conversation`, `body`/`text` | **Stub** — requires explicit approval, not yet wired to send. |

## Local Verification

Install Rust through `rustup`; this repository uses the checked-in
`rust-toolchain.toml` with stable Rust, `rustfmt`, and `clippy`.

On Debian or Ubuntu desktops and GitHub Actions runners, install the native
desktop build packages:

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libgl1-mesa-dev \
  libwayland-dev \
  libx11-dev \
  libxcb-keysyms1-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxcb1-dev \
  libxi-dev \
  libxkbcommon-dev \
  libxrandr-dev \
  pkg-config
```

Run the same verification sequence used by CI:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

`cargo build --locked` verifies the Linux desktop binary path. The CI workflow
does not require secrets and is safe to roll back by reverting the workflow and
toolchain/docs changes.
