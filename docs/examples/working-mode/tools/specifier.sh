#!/bin/sh
# Fixture specifier: reads IDEA.md.
# RESEARCH.md missing → write RESEARCH.md from IDEA and exit (no SPEC yet).
# Hello idea (second run) → SPEC/MAP/modules.
# Underspecified (e.g. saas/webapp, missing IDEA) → QUESTIONS.md, no MAP.md.
# Non-empty ANSWERS.md → SPEC/MAP (hello path). Kernel does not invent answers.
# Never ignores IDEA. Does not implement product. Does not write MAP-ACCEPT.
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

if [ -f IDEA.md ] && [ ! -f RESEARCH.md ]; then
  {
    printf '# RESEARCH\n\n'
    printf '## Stack survey\nlocal files from IDEA.md; no live services\n\n'
    printf '## Constraints\nLOW; no network; no credentials\n\n'
    printf '## Non-goals\nSPEC.md and MAP.md on this pass; product implementation; live tokens\n\n'
    printf '## Competitors\n'
    if grep -qi hello IDEA.md; then
      printf 'none known (hello product)\n'
    else
      printf 'unknown; idea not yet specified\n'
    fi
    printf '\n## Idea excerpt\n'
    cat IDEA.md
  } > RESEARCH.md
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    git add RESEARCH.md
    git diff --cached --quiet || git commit -qm 'specifier: RESEARCH'
  fi
  exit 0
fi

hello=0
if [ -f IDEA.md ] && [ -s IDEA.md ] && grep -qi hello IDEA.md; then
  hello=1
fi
if [ -s ANSWERS.md ]; then
  hello=1
fi

if [ "$hello" -eq 0 ]; then
  cat > QUESTIONS.md <<'EOF'
Who is the first user and what single job can they finish?
What is the first observable artifact (file, URL, or CLI) that proves it works?
Is v1 local-only, or does it need live network, deploy, or destroy?
Is auth or billing in scope for v1, or a non-goal?
Where does v1 data live (files, sqlite, none)?
Is this a LOW local map, or HIGH/live that needs MAP-HUMAN?
What is explicitly out of scope for this first map?
EOF
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    git add QUESTIONS.md
    git diff --cached --quiet || git commit -qm 'specifier: QUESTIONS'
  fi
  exit 0
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
