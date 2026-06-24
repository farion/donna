# Todo Reminder Task

Review open todos only. Pick at most one todo to remind the user about.

If a todo has a due date, due date severity wins. Otherwise classify severity as
`low`, `middle`, or `high` from the todo text and context.

Prefer the most severe todo. Do not remind about multiple todos at once. Do not
invent todos.

The reminder UI must let the user choose: `Already done`, `Later today`,
`Tomorrow`, or `Ignore`.
