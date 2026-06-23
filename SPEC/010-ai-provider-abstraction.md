# Story 010: AI Provider Abstraction

## User Story
As the user, I want Donna to work with local and remote AI providers, so that I can choose privacy, capability, and cost tradeoffs.

## Acceptance Criteria
- Donna defines a common AI provider interface.
- Ollama is supported.
- OpenAI-compatible APIs are supported.
- GitHub Copilot-compatible provider configuration is supported.
- Providers can be selected by model id.
- Provider errors are shown clearly.
- Streaming responses are supported or the architecture allows adding them.
- AI calls do not persist raw Donna chat by default.

## Notes
- The first implementation may include a mock provider for UI development.
