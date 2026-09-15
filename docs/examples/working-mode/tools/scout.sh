#!/bin/sh
# Fixture map-judge for the LOW hello-world example (not a live harness).
# Writes MAP-ACCEPT as this agent (must differ from MAPPER).
set -eu
agent=bob
if [ -f .wm/dispatch ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' .wm/dispatch)
  [ -n "$a" ] && agent=$a
fi
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' "$BRIEF")
  [ -n "$a" ] && agent=$a
fi
mkdir -p .wm/return
printf 'WORD: MAP-ACCEPT\nAGENT: %s\nMAP: MAP.md\n' "$agent" > ".wm/return/${agent}.md"
