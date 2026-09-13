#!/bin/sh
# Fixture reviewer for the LOW hello-world example (not a live harness).
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return reviews
ev=$(.wm/bin/wm evidence dave -- sh -c "$(sed -n 1p .wm/FALSIFIER)")
word=PASS
if [ -f .wm/red.status ] && [ "$(cat .wm/red.status)" = no-build ]; then
  word=NO-BUILD
fi
if [ "$word" = PASS ]; then
  cat > reviews/review.md <<'REV'
## Code
scope ok
## Testing
re-run named falsifier
REV
fi
printf 'WORD: %s\nEVIDENCE: %s\n' "$word" "$ev" > .wm/return/dave.md
