#!/bin/sh
# Run the README "## Smoke test" fenced ```sh block verbatim in a scratch flow,
# then assert the item closed and that a post-edit check refuses (A5).
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/crucible-quickstart.XXXXXX")

# First ``` fence in README is the loop diagram; first ```sh is the selftest
# one-liner. A5 is the multi-line smoke under "## Smoke test".
awk '
  /^## Smoke test$/ { section = 1; next }
  section && /^```sh$/ { copying = 1; next }
  copying && /^```$/ { exit }
  copying { print }
' "$root/README.md" > "$tmp/smoke.sh"

test -s "$tmp/smoke.sh"

# Assertions must run inside the smoke's $d after close; the fence cds away.
cat >> "$tmp/smoke.sh" <<'PROBE'

grep -q '^STATUS: CLOSED ' items/demo/ITEM.md \
  || { echo "A5-FAIL item not closed"; exit 1; }
echo 'x = 2' >> items/demo/work/a.py
if ./crucible check demo >/dev/null 2>&1; then
  echo "A5-FAIL post-edit gate accepted"
  exit 1
fi
echo "A5-QUICKSTART-OK"
PROBE

(
  cd "$root"
  sh "$tmp/smoke.sh"
)
printf 'A5-QUICKSTART-OK %s\n' "$tmp"
