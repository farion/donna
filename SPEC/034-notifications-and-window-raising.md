# Story 034: Notifications And Window Raising

## User Story
As the user, I want Donna to notify and raise itself appropriately, so that important work events break through even after hiding the window.

## Acceptance Criteria
- Donna can show desktop notifications where supported.
- Donna can request attention or raise its window where supported.
- Donna handles Linux Wayland environments including Sway, Hyprland, GNOME, and Plasma as primary targets.
- Donna also supports Windows and macOS behavior where feasible.
- Donna documents that Wayland compositors may prevent forced focus.
- Important events use the attention state.

## Notes
- Tray support is optional and not required for v1.
