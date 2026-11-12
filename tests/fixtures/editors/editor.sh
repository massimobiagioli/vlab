#!/bin/sh
# Test editor for `--edit-settings`. vlab runs it directly, never through a
# shell command string. Behavior is controlled by environment variables:
#   EDITOR_LOG     append each received argument, one per line
#   EDITOR_CAPTURE copy the file as received to this path
#   EDITOR_SOURCE  copy this file over the edited file
#   EDITOR_EXIT    exit with this status (default 0)

target=
for argument in "$@"; do
    target=$argument
    if [ -n "$EDITOR_LOG" ]; then
        printf '%s\n' "$argument" >> "$EDITOR_LOG"
    fi
done

if [ -n "$EDITOR_CAPTURE" ]; then
    cat "$target" > "$EDITOR_CAPTURE"
fi

if [ -n "$EDITOR_SOURCE" ]; then
    cat "$EDITOR_SOURCE" > "$target"
fi

exit "${EDITOR_EXIT:-0}"
