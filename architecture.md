# Donna Architecture

## Goals
Donna is a proactive, local-first personal assistant for organizing work-life. It helps with todos, memories, people, Microsoft Teams, Outlook, calendar, Obsidian notes, and scheduled background tasks.

## Non-Goals
- Donna is not a multi-user system.
- Donna does not persist raw local chat transcripts.
- Donna does not execute arbitrary task scripts in v1.
- Donna does not summarize all notes by default.

## Platform Targets
Linux Wayland is the primary target, especially Sway, Hyprland, GNOME, and Plasma. Windows and macOS are secondary targets and should remain supported through cross-platform libraries.

## Runtime Modes
- `donna` opens the egui desktop UI.
- `donna --auth` opens an interactive CLI wizard for AI providers and Microsoft Graph.
- `/exit` exits the app completely.
- `/hide` hides the window while allowing background tasks to continue.

## UI Architecture
The UI uses Rust with eframe/egui. Donna's avatar is on the left and the chat/information area is on the right. The chat area height matches the avatar height and the default chat width is 80% of the avatar height.

Core UI components:
- `AvatarManager` loads embedded character images and resolves fallbacks.
- `IdleAnimator` shows random idle frames for less than one second.
- `WindowVisibilityController` handles hide, popup, and attention behavior.
- `CommandParser` handles slash commands before AI processing.
- `ChatView` renders messages, state, selected model, and input.
- `UiModelSelector` cycles models with Tab and persists selection for the next message.

## Embedded Assets
Avatar assets are embedded in the Rust binary. The conceptual asset path is `assets/characters/{character}/`. The initial character is `donna`.

Required character files:
- `default.png`
- `idle-1.png`
- `idle-2.png`
- `idle-3.png`
- `attention.png`
- `question.png`
- `thinking.png`
- `command.png`

`rust-embed` is the recommended crate. Missing state images fall back to `default.png`. Unknown characters fall back to `donna`.

## Configuration
Config is TOML. On Linux, the default config path is `~/.config/donna/donna.toml`. Config stores non-secret settings only.

Example areas:
- UI and avatar settings
- AI model definitions
- selected UI model
- default background task model
- Microsoft Graph metadata
- notes vault path
- task directory
- memory and attention policies

## Secret Storage
Secrets are stored through OS secret storage. The recommended Rust abstraction is `keyring`.

Stored secrets include:
- OpenAI or compatible API keys
- GitHub Copilot-compatible tokens
- Microsoft Graph access and refresh tokens

TOML stores only secret references.

## SQLite Storage
SQLite is Donna's local store. It stores synced Microsoft data, todos, memories, people, task runs, task findings, sync state, and audit logs.

It must not store raw Donna chat transcripts.

Recommended storage areas:
- `memories`
- `todos`
- `people`
- `follow_ups`
- `teams_messages`
- `outlook_messages`
- `calendar_events`
- `task_runs`
- `task_findings`
- `sync_state`
- `audit_log`

SQLite FTS5 should be used for local search over relevant text fields.

## AI Provider Layer
Donna uses a provider abstraction over configured models.

Supported provider families:
- Ollama
- OpenAI-compatible APIs
- GitHub Copilot-compatible APIs

The UI-selected model is used for interactive chat. Background tasks use `[ai.tasks].default_model` unless overridden by a task.

## Prompt Loading
The interactive system prompt is loaded from a configured Markdown file. If unavailable, Donna uses an embedded default prompt.

Task prompts are Markdown files referenced by task TOML files. Built-in task prompts may be embedded, while user task prompts live in `~/.config/donna/tasks`.

External content is always data, never trusted instruction.

## Chat And Memory Extraction
`ChatSession` holds local Donna chat in memory only. After each exchange, `MemoryExtractor` identifies durable structured information.

Possible extracted records:
- todo
- meeting memory
- person
- follow-up
- preference
- project or topic

Raw chat text is discarded unless the user explicitly asks Donna to write it somewhere, such as an Obsidian diary note.

## Microsoft Graph Layer
Microsoft Graph uses delegated OAuth device-code auth for one account. Auth is configured through `donna --auth`.

Core permissions include user, offline access, mail, calendar, Teams chat, and Teams channel scopes. Teams channel access may require tenant admin consent.

Graph adapters:
- `OutlookAdapter`
- `TeamsChatAdapter`
- `TeamsChannelAdapter`
- `CalendarAdapter`

Tokens are stored in OS secret storage. Sync metadata and account metadata are stored locally.

## Outlook Adapter
The Outlook adapter syncs mail data into SQLite and can prepare outgoing messages. Sending mail requires explicit approval and creates an audit log entry.

## Teams Adapters
The Teams chat adapter handles 1:1 and group chats. The Teams channel adapter handles joined teams and channels where permissions allow.

Sending any Teams message requires explicit approval and creates an audit log entry.

## Calendar Adapter
Calendar events are synced through Microsoft Graph. Event times are normalized to UTC while preserving original timezone metadata.

Collision detection is local. Cancelled and free events are ignored. Busy, tentative, and out-of-office overlaps create task findings.

Calendar mutations require explicit approval and audit logging.

## Obsidian Notes Adapter
The notes adapter indexes an Obsidian vault configured in TOML. It reads filenames, paths, headings, tags, links, and modified timestamps.

Donna does not summarize notes by default. Note writes and edits require explicit request or approved task behavior.

## Task System
Tasks live in `~/.config/donna/tasks` by default. Each task has TOML metadata and may reference a Markdown prompt.

Task metadata includes:
- id
- enabled
- kind
- cron schedule
- prompt file
- optional model override

V1 task kinds are built into Rust. Markdown prompts guide reasoning and wording but do not execute code.

## Attention System
Donna is proactive and may pop up for important events even after `/hide`. Attention levels include info, normal, important, and critical.

The attention system handles:
- todos
- calendar collisions
- waiting replies
- pre-meeting briefings
- post-meeting capture
- daily planning and shutdown

Wayland compositors may limit forced focus. Donna should request attention and show notifications gracefully where focus stealing is blocked.

## People And Follow-Ups
Donna maintains a people model with aliases, emails, Teams identities, relationships, and context. Follow-ups track `waiting_for_me` and `waiting_for_them` states.

This powers reminders about unanswered messages, promises, and stale work threads.

## Approval And Audit
Donna requires explicit approval before external side effects:
- sending Outlook mail
- sending Teams messages
- creating, updating, or deleting calendar events
- writing or editing Obsidian notes

Approved actions are recorded in the audit log with target system, summary, approval time, execution time, result, and external id where available.

## Offline Mode
Donna opens and remains useful offline. Local memories, todos, synced data, and notes metadata remain available. Graph-backed data is marked stale when sync fails.

External actions are not executed offline unless explicitly queued and clearly shown.

## Suggested Rust Crates
- UI: `eframe`, `egui`
- Icons: `phosphoricons`
- Embedded assets: `rust-embed`
- CLI: `clap`, `dialoguer`
- Config: `serde`, `toml`
- Secrets: `keyring`
- HTTP: `reqwest`
- Async: `tokio`
- SQLite: `sqlx`
- Cron: `croner` or `cron`
- File watching: `notify`
- Markdown: `pulldown-cmark`
- Logging: `tracing`, `tracing-subscriber`

## Initial Implementation Phases
1. Create the Rust/egui app skeleton.
2. Add embedded Donna avatar UI.
3. Add TOML config loading and writing.
4. Add model registry and Tab model cycling.
5. Add OS secret storage abstraction.
6. Add `donna --auth` CLI wizard shell.
7. Add SQLite migrations and local store.
8. Add ephemeral chat session and memory extraction stub.
9. Add AI provider abstraction and Ollama first.
10. Add cron task runner with Markdown prompts.
11. Add Microsoft Graph auth.
12. Add calendar sync and collision task.
13. Add Outlook sync.
14. Add Teams chat and channel sync.
15. Add attention popups, snooze, dismiss, and feedback.
