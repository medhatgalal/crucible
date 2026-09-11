#!/bin/sh
# Fixture reviewer for the LOW hello-world example (not a live harness).
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence dave -- sh -c "$(sed -n 1p .wm/FALSIFIER)")
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/dave.md
