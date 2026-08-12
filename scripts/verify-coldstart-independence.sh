#!/bin/sh
# Cold-start independence contract: configure gate, role casting, ladder,
# solo-theatre refusal, and docs SSOT signals on public entrypoints.

set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
PASS=0; FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  if printf '%s\n' "$out" | grep -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}
refuses() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  if printf '%s\n' "$out" | grep -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}

write_agents() {
  prog=$1
  {
    printf 'c0\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'a1\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'a2\tkindB\tm\thigh\techo {BRIEF}\n'
    printf 'mk1\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'j1\tkindB\tm\thigh\techo {BRIEF}\n'
    printf 'j2\tkindB\tm\thigh\techo {BRIEF}\n'
  } > "$prog/agents.tsv"
}


seal_claim_agent() {
  # After claim dispatch, seal the independence ledger attempt for that agent.
  prog=$1; agent=$2; transport=${3:-multi-agent}
  id=
  for ad in "$prog"/attempts/A*; do
    [ -d "$ad" ] || continue
    aid=${ad##*/}
    a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
    [ "$a" = "$agent" ] || continue
    id=$aid
  done
  [ -n "$id" ] || { echo "no claim attempt for $agent" >&2; return 1; }
  auditor=$(awk -F '\t' '$1=="contract-auditor"{print $2; exit}' "$prog/PANEL.ASSIGN.tsv")
  [ -n "$auditor" ] || auditor=j2
  "$prog/crucible" attempt transport "$id" "$transport" >/dev/null
  "$prog/crucible" contract-audit "$id" "$auditor" PASS >/dev/null
}

write_panel() {
  prog=$1
  cat > "$prog/PANEL.md" <<'EOF'
# Panel
## Agents
c0, a1, a2, mk1, j1, j2
## Roles
See PANEL.ASSIGN.tsv casting table.
## Risk posture
LOW
## Isolation transport
multi-agent preferred; acp before subagent
## Independence ladder
1 multi-agent 2 acp 3 subagent 4 stop
## Waivers
NONE
EOF
  cat > "$prog/PANEL.ASSIGN.tsv" <<'EOF'
role	agent	required	notes
coordinator	c0	yes
claim-auditor	a1	yes
claim-auditor	a2	yes
scout	a1	no
maker	mk1	yes
reviewer	j1	yes
contract-auditor	j2	yes
EOF
}

# --- Docs SSOT (static) ---
if grep -q 'Do not conduct a long setup interview' "$HERE/BOOTSTRAP.md"; then
  bad 'BOOTSTRAP still encodes anti-interview lock-in'
else
  ok
fi
grep -qi 'role casting\|Role casting\|PANEL.ASSIGN' "$HERE/BOOTSTRAP.md" \
  && ok || bad 'BOOTSTRAP missing role casting / PANEL.ASSIGN'
grep -qi 'role casting\|Role casting\|PANEL.ASSIGN' "$HERE/START.md" \
  && ok || bad 'START missing role casting / PANEL.ASSIGN'
grep -qi 'independence ladder\|Independence ladder' "$HERE/BOOTSTRAP.md" && ok || bad 'BOOTSTRAP missing ladder'
grep -q 'contract-auditor' "$HERE/START.md" && ok || bad 'START missing contract-auditor'
grep -q 'approve-panel' "$HERE/docs/managed-lifecycle.md" && ok || bad 'managed guide missing approve-panel'
[ -f "$HERE/roles/contract-auditor.md" ] && ok || bad 'contract-auditor role missing'

# --- Behavioral gates ---
base=$(mktemp -d "${TMPDIR:-/tmp}/crucible-coldstart-ind.XXXXXX")
repo="$base/repo"; mkdir -p "$repo"
(
  cd "$repo"
  git init -q -b main
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
  "$C" adopt work --managed >/dev/null
)
P="$repo/.crucible/work"

expect 'guided cold start opens at CONFIGURE' '^NEXT CONFIGURE ' "$P/crucible" cycle
printf 'problem\n' > "$repo/report.md"
refuses 'cannot bind problem before panel' 'approve the agent panel' \
  "$P/crucible" cycle problem "$repo/report.md"
refuses 'cannot claim-add before panel' 'approve the agent panel' \
  "$P/crucible" claim add 'x' 'y'

write_agents "$P"
cat > "$P/PANEL.md" <<'EOF'
# Panel
## Agents
a1
## Roles
vague
## Risk posture
LOW
## Isolation transport
multi-agent
## Independence ladder
1 multi-agent
## Waivers
NONE
EOF
refuses 'panel without ASSIGN refuses' 'ASSIGN\|casting\|incomplete' \
  "$P/crucible" cycle approve-panel

write_panel "$P"
write_agents "$P"
expect 'panel approves with casting' '^approved panel ' "$P/crucible" cycle approve-panel
expect 'after panel, intake next' '^NEXT INTAKE ' "$P/crucible" cycle
$P/crucible cycle problem "$repo/report.md" >/dev/null
expect 'after problem, investigate' '^NEXT INVESTIGATE ' "$P/crucible" cycle

# Claim TRUE without dispatch refuses; with dispatch succeeds.
cn=$($P/crucible claim add 'x' 'x source')
$P/crucible run-claim "$cn" a1 -- sh -c 'echo e' >/dev/null
refuses 'TRUE without claim dispatch' 'claim dispatch' "$P/crucible" claim verdict "$cn" a1 TRUE
$P/crucible dispatch "$cn" claim-auditor a1 >/dev/null
seal_claim_agent "$P" a1
if $P/crucible claim verdict "$cn" a1 TRUE >/dev/null; then ok; else bad 'TRUE with dispatch should pass'; fi

# ACP probe helper: failed alone ok; cannot unbound-downgrade after ok
expect 'probe-acp writes record' 'ACP-PROBE.md' "$P/crucible" probe-acp failed 'no acp in fixture'
grep -q '^status: failed$' "$P/ACP-PROBE.md" && ok || bad 'ACP-PROBE status not failed'
# fresh ok then refuse failed downgrade
rm -f "$P/ACP-PROBE.md"
rm -rf "$P/acp-probes"
expect 'probe-acp ok' 'ACP-PROBE.md' "$P/crucible" probe-acp ok 'acp works'
refuses 'cannot downgrade ok probe to failed' 'cannot downgrade' \
  "$P/crucible" probe-acp failed 'spoof'

# agents.tsv mutation invalidates panel approval (registry is in panel-id)
printf 'extra\tkindB\tm\thigh\techo x\n' >> "$P/agents.tsv"
expect 'agents.tsv drift invalidates panel' '^WAIT PANEL ' "$P/crucible" cycle
write_agents "$P"
expect 'restoring agents.tsv restores prior panel approval' '^NEXT ' "$P/crucible" cycle

# Panel edit invalidates approval
printf '\n# note\n' >> "$P/PANEL.md"
expect 'panel drift returns to WAIT PANEL' '^WAIT PANEL ' "$P/crucible" cycle

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
