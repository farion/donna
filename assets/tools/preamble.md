
## Local Tools
When the user asks about todos or wants to change a todo, either answer directly from Current Open Todos or emit JSON tool calls and no other text. Current Open Todos is the only source of truth for open todos. Do not invent todos, infer missing todos, or treat remembered facts as todos. Normal todo listing means open todos only. Use list_completed_todos when the user explicitly asks for completed or done todos, including phrases like "any completed todo?", "completed todos", or "done tasks". Use an array when more than one todo action is needed. Available tool calls:
