# Donna MVP User Guide

Donna is a single-user, local-first personal work-life assistant built with Rust
and egui. The MVP is an assistant shell with local state, structured memory and
todo extraction, Microsoft Graph authentication foundations, Microsoft sync and
approved-action adapters, Obsidian metadata indexing, and background-task
configuration.

Donna is designed around one important privacy rule: raw Donna chat transcripts
stay in memory for the current app session only. Durable storage is limited to
structured records such as memories, todos, people, task findings, synced
Microsoft data, note metadata, sync state, offline state, and audit records.

## Install And Run

Install Rust with `rustup`. The repository pins stable Rust through
`rust-toolchain.toml`.

On Debian or Ubuntu, install the native desktop dependencies used by the Linux
build:

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

Build Donna, or run the desktop UI directly from the repository:

```sh
cargo build --locked
cargo run --locked
```

After installing the binary on your `PATH`, run:

```sh
donna
```

Donna opens a desktop window titled `Donna`. On first launch it creates
`~/.config/donna/donna.toml` on Linux and opens a SQLite database under
`~/.local/share/donna/` unless you configure another path.

## First-Run Checklist

1. Start Donna once so it can create the default TOML config.
2. Review `~/.config/donna/donna.toml`.
3. Choose or edit AI model entries under `[ai]`.
4. Run `donna --auth` if you want Microsoft Graph auth configured.
5. Add task TOML files under `~/.config/donna/tasks/` if you want background
   task definitions available.
6. Set `[notes].obsidian_vault_path` if you want Obsidian metadata indexing.

## Desktop UI

The MVP desktop UI has an avatar area and a chat panel.

- The chat bar shows Donna's current state, storage/offline/stale-data status,
  and the selected chat model.
- User and Donna messages use different bubble colors from the TOML config.
- Press `Enter` or click `Send` to submit a message.
- Press `Tab` to cycle configured chat models. Donna saves the selected model id
  back to TOML.
- Donna follows the configured UI theme: `auto`, `light`, or `dark`.

Donna currently responds in shell mode. It records your message in the in-memory
session, attempts structured memory/todo/person extraction, and replies with the
selected model label plus a storage/privacy note. The chat UI does not yet route
normal conversation through a live AI provider response.

## Commands

Supported commands:

- `/exit` asks for confirmation before closing Donna. Use `/exit confirm` to
  close the window and stop the in-process task runner state.
- `/hide` asks the desktop environment to minimize Donna. Some Wayland
  compositors may ignore app-driven minimize requests; Donna shows a fallback
  note and keeps running.
- `/changechar [name]` changes to an embedded avatar character and persists the
  selection in `donna.toml`.
- `/theme <mode>` changes the UI theme and persists it in `donna.toml`. Supported
  modes are `auto`, `light`, and `dark`.

Commands are handled locally before AI processing and do not enter the visible
chat timeline. When the input starts with `/`, Donna shows compact suggestions
for the supported commands. Unknown commands show an inline error near the input.

## Configuration

Donna uses TOML for non-secret settings. On Linux the default config path is:

```text
~/.config/donna/donna.toml
```

Secrets do not belong in TOML. Store API keys and Microsoft tokens in OS secret
storage and reference them by name from TOML.

Example MVP config:

```toml
[ui]
theme = "auto"
donna_message_color = "#eef5ff"
user_message_color = "#eaf7ef"

[avatar]
character = "donna"

[ai.chat]
selected_model = "ollama-local"

[ai.tasks]
default_model = "ollama-local"

[[ai.models]]
id = "ollama-local"
label = "Ollama local"
provider = "ollama"
model = "llama3.1"
base_url = "http://localhost:11434"

[[ai.models]]
id = "openai-compatible"
label = "OpenAI compatible"
provider = "openai-compatible"
model = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
secret_ref = "donna/openai"

[[ai.models]]
id = "github-copilot-compatible"
label = "GitHub Copilot compatible"
provider = "github-copilot-compatible"
model = "gpt-4.1"
base_url = "https://api.githubcopilot.com"
secret_ref = "donna/github-copilot"

[microsoft]
tenant_id = "common"
scopes = [
  "User.Read",
  "offline_access",
  "Mail.Read",
  "Mail.Send",
  "Calendars.ReadWrite",
  "ChatMessage.Send",
  "Chat.ReadWrite",
  "ChannelMessage.Read.All",
  "ChannelMessage.Send",
  "Team.ReadBasic.All",
]

[notes]
# obsidian_vault_path = "/home/you/Notes"

[prompts]
system_prompt_path = "/home/you/.config/donna/prompts/system.md"

[data]
database_path = "/home/you/.local/share/donna/donna.sqlite3"
stale_after_minutes = 60

[tasks]
directory = "/home/you/.config/donna/tasks"

[memory]
require_sensitive_approval = true

[attention]
enabled = true
notification_min_level = "normal"
popup_min_level = "important"
popup_cooldown_seconds = 900
critical_bypasses_cooldown = true

[offline]
show_stale_data_warnings = true
queue_external_actions = false
```

Paths in the generated config use your actual home directory. If the config file
is invalid TOML, Donna falls back to defaults for that launch and shows an error
notice in the app.

The `[ui].theme` value accepts:

- `auto`: follow the operating system theme when available.
- `light`: force Donna's light theme.
- `dark`: force Donna's dark theme.

## AI Providers

Donna's model list is configurable. Each model entry has:

- `id`: stable id used by model selection and task configs.
- `label`: readable label shown in the chat bar.
- `provider`: provider family.
- `model`: provider model name.
- `base_url`: provider endpoint.
- `secret_ref`: optional OS secret storage reference.

Supported provider families in the config model are:

- `ollama`
- `openai-compatible`
- `github-copilot-compatible`
- `mock` for tests and internal development

The MVP has a concrete Ollama request adapter. OpenAI-compatible and
GitHub-Copilot-compatible entries can be configured and selected, but their HTTP
adapters are not wired into normal chat yet.

For local Ollama, keep the default model or edit it to match a model you have
pulled:

```sh
ollama pull llama3.1
ollama serve
```

Task execution uses `[ai.tasks].default_model` and per-task overrides, not the
currently selected chat model. This keeps background work stable when you cycle
the chat model in the UI.

## Chat Privacy And Structured Records

Donna's chat session lives in memory only. Closing the app discards the raw chat
messages.

When you send a message, Donna may extract structured records:

- `todo: file receipts`
- `remember to send the weekly note`
- `need to renew the certificate`
- `meeting with Anna about billing`
- `I prefer short updates`
- `remember that the router is in the closet`

Stored records use a source such as `donna_chat`, but they do not store the raw
message as a transcript. Sensitive-looking memories containing words such as
`password`, `secret`, `token`, `ssn`, `medical`, `health`, or `salary` require
review. Donna keeps them as structured drafts in the current session until you
save, edit, or delete them from the sensitive memory review card.

Donna stores structured data in SQLite tables for memories, todos, people,
follow-ups, task runs, task findings, synced Microsoft data, notes metadata, sync
state, local state, attention items, audit log, and search index. The schema does
not create raw Donna chat transcript tables.

## Tasks And Markdown Prompts

Donna reads task definitions from:

```text
~/.config/donna/tasks/
```

Each task definition is a TOML file. Prompt files are Markdown and are treated as
prompts only; they do not execute code.

Example task file:

```toml
id = "weekday-daily-plan"
enabled = true
kind = "daily_planning"
cron = "0 8 * * 1-5"
prompt_file = "daily-plan.md"
model = "ollama-local"
```

Example Markdown prompt next to the task file:

```md
# Weekday Daily Plan

Review local todos, follow-ups, calendar context, and stale-data warnings.
Produce structured findings only. Do not send messages or modify external
systems.
```

Task TOML fields:

- `id`: required stable task id.
- `enabled`: optional, defaults to `true`.
- `kind`: task kind. Built-in names include `daily_planning`, `shutdown_review`,
  `calendar_collision`, and `mail_follow_up`; other names are loaded as generic
  task kinds.
- `cron`: required five-field cron expression: minute, hour, day of month,
  month, weekday.
- `prompt_file`: optional Markdown file. Relative paths resolve from the task
  TOML file's directory.
- `model`: optional model id override.

If `prompt_file` is missing or unreadable, Donna falls back to the embedded
default task prompt and records a notice. The task planning core can identify due
tasks and choose the task model; the MVP desktop UI does not yet expose a task
console.

## Microsoft Graph Auth

Run the auth wizard:

```sh
donna --auth
```

The wizard asks for:

- Microsoft app client id
- tenant id, defaulting to `common`
- optional account hint
- token secret reference, defaulting to `donna/microsoft`

Donna saves only non-secret Microsoft metadata to `donna.toml`. It then starts
delegated device-code auth, shows Microsoft's verification URL and user code,
polls for the token, and stores token JSON in OS secret storage.

Default Graph scopes:

- `User.Read`
- `offline_access`
- `Mail.Read`
- `Mail.Send`
- `Calendars.ReadWrite`
- `ChatMessage.Send`
- `Chat.ReadWrite`
- `ChannelMessage.Read.All`
- `ChannelMessage.Send`
- `Team.ReadBasic.All`

Some tenants require administrator consent for these scopes. If Microsoft
returns an admin-consent error, Donna reports that tenant admin consent is needed.
Some Teams permissions can also be unavailable or unconsented in a tenant; Donna
reports those as Teams Graph permission problems rather than hiding the cause.

## Microsoft Sync And Actions

The MVP includes adapter foundations for:

- Outlook mail sync and approved send-mail actions.
- Teams chat and channel sync plus approved send-message actions.
- Calendar sync, collision checks, and approved create/update/delete actions.

Synced Microsoft content is stored locally in SQLite with external ids, sync
state, deletion flags, and search records. Microsoft, calendar, mail, Teams, and
notes-derived search results are treated as untrusted external data. They can
inform analysis, but they cannot override Donna's system prompt, task prompts, or
approval gates.

Donna requires explicit approval before:

- sending Outlook mail
- sending Teams chat or channel messages
- creating, updating, or deleting calendar events
- writing or editing notes

Approved external actions create audit-log records with the action type, target
system, summary, approval time, execution time, result, and external id when
available.

Donna does not execute Microsoft Graph sync or external send/change actions while
the local store is offline. Failed sync attempts mark the source as stale and
preserve the last cursor or delta link for a later retry.

## Obsidian Notes

Set an Obsidian vault path in TOML:

```toml
[notes]
obsidian_vault_path = "/home/you/Notes"
```

Donna's Obsidian indexer walks Markdown files in the vault and stores metadata:

- note path
- title
- headings
- tags
- wiki links and Markdown links
- modified time

The indexer reads note files locally to derive metadata, but it stores metadata
rather than note bodies. Writing or editing notes remains an approval-gated
external action.

## Search, Trust, And External Content

Donna maintains a local FTS search index for structured records and synced or
indexed external data.

Search trust labels are internal, but the rule is simple:

- local structured records such as Donna memories and todos are trusted local
  structured data.
- Outlook, Teams, calendar, and Obsidian-derived records are untrusted external
  data.

Untrusted external text may contain useful facts, but Donna must treat it as data
only. It cannot change Donna's instructions, disable approval gates, or force
side effects.

## Offline And Stale Data

Donna keeps local data available when network-dependent systems are unavailable.
The chat bar makes offline and stale-data states visible:

- `Offline` means the local store is marked offline. Microsoft sync and external
  actions are paused.
- `Stale: Mail`, `Stale: Teams`, or `Stale: Calendar` means the last sync for
  that source failed or was explicitly marked stale.
- If storage cannot be opened, the status reads `Storage unavailable`.

With `queue_external_actions = false` in `[offline]`, Donna does not queue
offline mail, Teams, calendar, or note writes. It requires a fresh approved
action when the system is online again.

## Avatars

Donna embeds avatar assets in the binary. The default character is `donna`.

Embedded state images include:

- default
- idle frames
- attention
- question
- thinking
- command

The current UI uses idle frames during normal chat, command state for command
input, thinking/question states for active response or approval work, and
attention state for active attention items or when `/hide` is requested. Missing
characters or missing state images fall back to Donna's default image.

To change the configured character manually:

```toml
[avatar]
character = "donna"
```

Only embedded character asset folders are available in the MVP.

## Troubleshooting

`invalid config TOML`

Fix `~/.config/donna/donna.toml`. Donna uses defaults for the current launch if
the file cannot be decoded.

`Storage unavailable`

Check `[data].database_path` and parent directory permissions. Donna creates the
database directory automatically when it can.

`Offline`

Donna's local store is marked offline. Microsoft sync and external Graph actions
will not run in this state.

`Stale: Mail`, `Stale: Teams`, or `Stale: Calendar`

The last sync for that source failed or was skipped while offline. Treat local
results from that source as potentially outdated until a later sync succeeds.

Microsoft admin consent error

Ask the tenant administrator to consent to the configured delegated scopes, or
reduce the scopes in `[microsoft].scopes` to the smallest set your tenant allows.

Teams permission unavailable

Some Teams Graph permissions require tenant configuration or may not be available
to the app. Donna reports these separately so you can distinguish Teams permission
issues from general auth failures.

Ollama connection error

Make sure Ollama is running and that the configured `base_url` uses plain
`http://`. The current Ollama adapter does not support HTTPS endpoints.

`Unknown avatar character: [name]`

Use `/changechar [name]` only with an embedded character folder. Donna keeps the
current configured avatar when the requested character is unavailable.

## Local Verification

Before handing Donna to another agent or packaging a release, run:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```
