#!/bin/sh
# Appends one line per lifecycle command to the fixture's hook log, so the order and repetition can be inspected.

mkdir -p "$PWD/.fixture"
printf '%s user=%s pwd=%s fixture=%s remote=%s\n' "$1" "$(id -un)" "$PWD" "${PANDEMONIUM_FIXTURE:-}" "${PANDEMONIUM_REMOTE:-}" >> "$PWD/.fixture/hooks.log"
