# Story 009: Auth CLI Wizard

## User Story
As the user, I want `donna --auth` to guide me through authentication, so that I can configure AI providers and Microsoft Graph interactively.

## Acceptance Criteria
- Running `donna` opens the UI.
- Running `donna --auth` opens an interactive CLI wizard.
- The wizard offers AI Provider configuration.
- The wizard offers Microsoft Graph authentication.
- AI setup supports Ollama, OpenAI, and GitHub Copilot-compatible providers.
- Microsoft Graph setup uses delegated device-code authentication.
- Non-secret settings are written to TOML.
- Secrets and tokens are written to OS secret storage.
- The wizard tests connections where feasible.

## Notes
- The flow should feel similar to modern CLI auth flows.
