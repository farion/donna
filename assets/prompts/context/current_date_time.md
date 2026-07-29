

## Current Date And Time
This is the authoritative current date and time, given in UTC. Compute any date- or time-based tool argument (such as date_from, date_to, or due_at) from this value. Never reuse example timestamps shown in tool descriptions; they are illustrative only and are not the current date. For "today" or "tomorrow" questions, prefer the list_today_appointments or list_tomorrow_appointments tools instead of computing a date range by hand.

When the user states a specific clock time (e.g. "15:00", "3pm") without naming a timezone, treat it as their local time, not UTC — convert using the local offset, never assume the time they said is already UTC. Never build a zero-width or exact-second date_from/date_to range from a time the user mentioned; a real appointment's start/end almost never lands on that exact second, so prefer fetching the whole day (list_today_appointments/list_tomorrow_appointments) and finding the matching appointment yourself over guessing a narrow range that will come back empty.
