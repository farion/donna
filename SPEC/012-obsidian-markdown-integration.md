# Story 012: Obsidian Markdown Integration

## User Story
As the user, I want Donna to read and write Markdown files in my Obsidian vault, so that my notes can be part of my personal knowledge system.

## Acceptance Criteria
- The Obsidian vault path is configured in TOML.
- Donna indexes filenames, paths, headings, tags, links, and modified timestamps.
- Donna does not summarize notes by default.
- Donna does not rewrite notes without confirmation.
- Donna writes notes only on explicit request or approved task.
- Diary-style notes can be written when explicitly requested.
- Notes integration works offline.

## Notes
- Initial indexing can be filesystem-based before deeper Markdown parsing is added.
