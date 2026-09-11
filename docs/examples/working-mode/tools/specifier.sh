#!/bin/sh
# Fixture specifier for the LOW hello-world example (not a live harness).
# Writes SPEC.md + architecture/modules.md TSV + MAP.md. MAPPER is this agent.
# Does not implement product. Does not write MAP-ACCEPT.
set -eu
agent=eve
if [ -f .wm/dispatch ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' .wm/dispatch)
  [ -n "$a" ] && agent=$a
fi
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' "$BRIEF")
  [ -n "$a" ] && agent=$a
fi
mkdir -p architecture product
cat > SPEC.md <<'EOF'
## Goal
product/hello.txt contains exactly hello
## Non-goals
live systems, extra modules, network
## Owned files
- product/hello.txt
## Test files
- (none)
## Acceptance criteria
- product/hello.txt contains exactly hello
- maker-authored falsifier is grep -qx hello product/hello.txt
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: specifier
EOF
cat > architecture/modules.md <<'EOF'
module_id	root_path	public_contracts	test_entrypoint	pattern_instance	live_write
product	product	product/hello.txt	product/hello.txt	product/hello.txt	no
EOF
{
  printf 'MAPPER: %s\n\n' "$agent"
  printf 'id\tmodule\towned_paths\tdepends_on\trisk\n'
  printf 's1\tproduct\tproduct/hello.txt\t-\tLOW\n'
} > MAP.md
git add SPEC.md architecture/modules.md MAP.md
if git rev-parse --verify HEAD >/dev/null 2>&1; then
  git diff --cached --quiet || git commit -qm 'specifier: SPEC MAP modules'
fi
