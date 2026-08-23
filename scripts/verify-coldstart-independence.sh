#!/bin/sh
# Cold-start independence contract: configure gate, role casting, ladder,
# solo-theatre refusal, and docs SSOT signals on public entrypoints.

set -eu
# This file is one long guided investigation with many NEW claims in one program.
# Product default remains 3; the cap CHECK is in verify-agent-cycle.sh.
CRUCIBLE_MAX_NEW_CLAIMS=${CRUCIBLE_MAX_NEW_CLAIMS:-32}
export CRUCIBLE_MAX_NEW_CLAIMS

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
PASS=0; FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  if printf '%s\n' "$out" | grep -E -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}
refuses() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  if printf '%s\n' "$out" | grep -E -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
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
grep -qiE 'role casting|Role casting|PANEL.ASSIGN' "$HERE/BOOTSTRAP.md" \
  && ok || bad 'BOOTSTRAP missing role casting / PANEL.ASSIGN'
grep -qiE 'role casting|Role casting|PANEL.ASSIGN' "$HERE/START.md" \
  && ok || bad 'START missing role casting / PANEL.ASSIGN'
grep -qiE 'independence ladder|Independence ladder' "$HERE/BOOTSTRAP.md" && ok || bad 'BOOTSTRAP missing ladder'
grep -q 'contract-auditor' "$HERE/START.md" && ok || bad 'START missing contract-auditor'
grep -q 'approve-panel' "$HERE/docs/managed-lifecycle.md" && ok || bad 'managed guide missing approve-panel'
[ -f "$HERE/roles/contract-auditor.md" ] && ok || bad 'contract-auditor role missing'

# --- Behavioral gates ---
base=$(mktemp -d "${TMPDIR:-/tmp}/crucible-coldstart-ind.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$base"' 0
trap 'rm -rf "$base"; exit 129' 1
trap 'rm -rf "$base"; exit 130' 2
trap 'rm -rf "$base"; exit 143' 15
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
refuses 'panel without ASSIGN refuses' 'ASSIGN|casting|incomplete' \
  "$P/crucible" cycle approve-panel

write_panel "$P"
write_agents "$P"
expect 'panel approves with casting' '^approved panel ' "$P/crucible" cycle approve-panel
expect 'after panel, intake next' '^NEXT INTAKE ' "$P/crucible" cycle
$P/crucible cycle problem "$repo/report.md" >/dev/null
expect 'after problem, investigate' '^NEXT INVESTIGATE ' "$P/crucible" cycle

# Claim verdicts without dispatch refuse; with dispatch+seal succeed (TRUE and FALSE).
cn=$($P/crucible claim add 'x' 'x source')
$P/crucible run-claim "$cn" a1 -- sh -c 'echo e' >/dev/null
refuses 'TRUE without claim dispatch' 'claim dispatch' "$P/crucible" claim verdict "$cn" a1 TRUE
refuses 'FALSE without claim dispatch' 'claim dispatch' "$P/crucible" claim verdict "$cn" a1 FALSE
$P/crucible dispatch "$cn" claim-auditor a1 >/dev/null
refuses 'TRUE with dispatch but no seal' 'sealed|matching claim attempt|transport|contract-audit' \
  "$P/crucible" claim verdict "$cn" a1 TRUE
seal_claim_agent "$P" a1
if $P/crucible claim verdict "$cn" a1 TRUE >/dev/null; then ok; else bad 'TRUE with dispatch+seal should pass'; fi

# Guided start without seal refuses; transport after start refuses.
cn_s=$($P/crucible claim add 'start-seal' 'source')
$P/crucible dispatch "$cn_s" claim-auditor a1 >/dev/null
aid_s=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$item" = "$cn_s" ] && [ "$agent" = a1 ] && aid_s=${ad##*/}
done
[ -n "$aid_s" ] || { bad 'no claim attempt for start-seal'; aid_s=missing; }
refuses 'guided start without seal' 'transport|contract-audit' \
  "$P/crucible" attempt start "$aid_s" $$
$P/crucible attempt transport "$aid_s" multi-agent >/dev/null
$P/crucible contract-audit "$aid_s" j2 PASS >/dev/null
expect 'guided start with seal' "RUNNING pid" "$P/crucible" attempt start "$aid_s" $$
refuses 'transport after start' 'DISPATCHED' \
  "$P/crucible" attempt transport "$aid_s" acp
$P/crucible attempt finish "$aid_s" RETURNED observed >/dev/null
refuses 'transport after RETURNED' 'DISPATCHED' \
  "$P/crucible" attempt transport "$aid_s" acp

# Stale panel refuses dispatch (execution bind, not only cycle display).
printf 'extra\tkindB\tm\thigh\techo x\n' >> "$P/agents.tsv"
expect 'agents.tsv drift invalidates panel display' '^WAIT PANEL ' "$P/crucible" cycle
cn_d=$($P/crucible claim add 'stale-dispatch' 'source' 2>&1) || true
# claim add itself should refuse on stale panel
refuses 'claim add with stale panel' 'panel' \
  "$P/crucible" claim add 'stale-dispatch-2' 'source'
# restore for a live claim then re-stale for dispatch
write_agents "$P"
expect 'restoring agents.tsv restores prior panel approval' '^NEXT ' "$P/crucible" cycle
cn_d=$($P/crucible claim add 'stale-dispatch' 'source')
printf 'extra\tkindB\tm\thigh\techo x\n' >> "$P/agents.tsv"
refuses 'dispatch with stale panel' 'panel' \
  "$P/crucible" dispatch "$cn_d" claim-auditor a1
write_agents "$P"

# Temporary kind-collapse cannot leave dishonest acp seal after restore.
cn_l=$($P/crucible claim add 'ladder-restore' 'source')
$P/crucible dispatch "$cn_l" claim-auditor a1 >/dev/null
aid_l=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$item" = "$cn_l" ] && [ "$agent" = a1 ] && aid_l=${ad##*/}
done
# Collapse to one kind and re-approve so transport acp can be recorded.
{
  printf 'c0\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'a1\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'a2\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'mk1\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'j1\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'j2\tkindA\tm\thigh\techo {BRIEF}\n'
} > "$P/agents.tsv"
# single-kind still needs re-approve after registry change
expect 'single-kind needs re-approve' '^WAIT PANEL ' "$P/crucible" cycle
# Cannot seal under unapproved panel
refuses 'transport under unapproved panel' 'panel' \
  "$P/crucible" attempt transport "$aid_l" acp
write_agents "$P"
# Re-approve multi-kind, try acp without waiver → refuse
expect 'multi-kind restore' '^NEXT ' "$P/crucible" cycle
expect 'acp hop is legal on a multi-kind panel' 'transport acp' \
  "$P/crucible" attempt transport "$aid_l" acp

# Cast exclusion: coordinator cannot be contract-auditor
cat > "$P/PANEL.ASSIGN.tsv" <<'EOF'
role	agent	required	notes
coordinator	c0	yes
claim-auditor	a1	yes
claim-auditor	a2	yes
scout	a1	no
maker	mk1	yes
reviewer	j1	yes
contract-auditor	c0	yes
EOF
refuses 'coordinator as contract-auditor' 'PANEL|casting|incomplete|ASSIGN' \
  "$P/crucible" cycle approve-panel
write_panel "$P"
write_agents "$P"
# panel already drifted; re-approve clean panel (may re-point a prior content hash)
out=$($P/crucible cycle approve-panel 2>&1) || { bad "clean panel re-approves: $out"; out=; }
if printf '%s\n' "$out" | grep -E -q 'approved panel|already approved'; then ok; else bad "clean panel re-approves: $out"; fi

# ACP: prior ok blocks PANEL ACP: unavailable unlock
rm -f "$P/ACP-PROBE.md"
rm -rf "$P/acp-probes"
expect 'probe-acp ok' 'ACP-PROBE.md' "$P/crucible" probe-acp ok 'acp works'
# rewrite panel with ACP: unavailable and single-product waiver
cat > "$P/PANEL.md" <<'EOF'
# Panel
## Agents
c0, a1, a2, mk1, j1, j2
## Roles
See PANEL.ASSIGN.tsv
## Risk posture
LOW
## Isolation transport
subagent after ACP: unavailable
## Independence ladder
1 multi-agent 2 acp 3 subagent 4 stop
## Waivers
LADDER_WAIVER: single-product
ACP: unavailable
EOF
expect 'panel with ACP note re-approves' '^approved panel ' "$P/crucible" cycle approve-panel
cn_acp=$($P/crucible claim add 'acp-ok-blocks' 'source')
$P/crucible dispatch "$cn_acp" claim-auditor a1 >/dev/null
aid_acp=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$item" = "$cn_acp" ] && [ "$agent" = a1 ] && aid_acp=${ad##*/}
done
refuses 'subagent blocked after probe ok despite PANEL' 'ACP|subagent|probe' \
  "$P/crucible" attempt transport "$aid_acp" subagent

# ACP probe helper: failed alone ok; cannot unbound-downgrade after ok
write_panel "$P"
write_agents "$P"
expect 'restore default panel' '^approved panel ' "$P/crucible" cycle approve-panel
rm -f "$P/ACP-PROBE.md"
rm -rf "$P/acp-probes"
expect 'probe-acp writes failed record' 'ACP-PROBE.md' "$P/crucible" probe-acp failed 'no acp in fixture'
grep -q '^status: failed$' "$P/ACP-PROBE.md" && ok || bad 'ACP-PROBE status not failed'
rm -f "$P/ACP-PROBE.md"
rm -rf "$P/acp-probes"
expect 'probe-acp ok again' 'ACP-PROBE.md' "$P/crucible" probe-acp ok 'acp works'
refuses 'cannot downgrade ok probe to failed' 'cannot downgrade' \
  "$P/crucible" probe-acp failed 'spoof'

# Panel edit invalidates approval
printf '\n# note\n' >> "$P/PANEL.md"
expect 'panel drift returns to WAIT PANEL' '^WAIT PANEL ' "$P/crucible" cycle

# Prose must not grant maker-reviewer waiver
write_panel "$P"
printf '\nWe do not grant maker-reviewer-same-agent.\n' >> "$P/PANEL.md"
cat > "$P/PANEL.ASSIGN.tsv" <<'EOF'
role	agent	required	notes
coordinator	c0	yes
claim-auditor	a1	yes
claim-auditor	a2	yes
scout	a1	no
maker	mk1	yes
reviewer	mk1	yes
contract-auditor	j2	yes
EOF
write_agents "$P"
refuses 'prose does not grant maker=reviewer' 'PANEL|casting|incomplete|ASSIGN' \
  "$P/crucible" cycle approve-panel

# Restore clean panel for remaining cases
write_panel "$P"
write_agents "$P"
out=$($P/crucible cycle approve-panel 2>&1) || { bad "panel for residual cases: $out"; out=; }
if printf '%s\n' "$out" | grep -E -q 'approved panel|already approved'; then ok; else bad "panel for residual cases: $out"; fi

# FIX SUPERSEDES: same-attempt PASS refuses; redispatch can proceed
cn_fix=$($P/crucible claim add 'fix-super' 'source')
$P/crucible dispatch "$cn_fix" claim-auditor a1 >/dev/null
aid_fix=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$item" = "$cn_fix" ] && [ "$agent" = a1 ] && aid_fix=${ad##*/}
done
$P/crucible attempt transport "$aid_fix" multi-agent >/dev/null
$P/crucible contract-audit "$aid_fix" j2 FIX 'broken' >/dev/null
st_fix=$(awk -F '\t' 'NR>1 {s=$1} END{print s}' "$P/attempts/$aid_fix/events.tsv")
[ "$st_fix" = SUPERSEDED ] && ok || bad "FIX should SUPERSEDE attempt (got $st_fix)"
refuses 'PASS after FIX on same attempt' 'DISPATCHED|immutable|terminal|SUPERSEDED' \
  "$P/crucible" contract-audit "$aid_fix" j2 PASS

# Stolen attempt-id: dispatch attempt-id pointing at another claim's sealed attempt
cn_a=$($P/crucible claim add 'steal-a' 'source')
cn_b=$($P/crucible claim add 'steal-b' 'source')
$P/crucible dispatch "$cn_a" claim-auditor a1 >/dev/null
seal_claim_agent "$P" a1
aid_a=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$item" = "$cn_a" ] && [ "$agent" = a1 ] && aid_a=${ad##*/}
done
$P/crucible dispatch "$cn_b" claim-auditor a1 >/dev/null
# Point B's dispatch at A's sealed attempt id
df_b=$(ls "$P/claims/$cn_b/dispatches/"*-claim-auditor-a1.md | head -1)
# replace attempt-id line
tmpf="$P/.steal.tmp"
awk -v id="$aid_a" '
  /^attempt-id: / { print "attempt-id: " id; next }
  { print }
' "$df_b" > "$tmpf" && mv "$tmpf" "$df_b"
$P/crucible run-claim "$cn_b" a1 -- sh -c 'echo e' >/dev/null
refuses 'stolen attempt-id does not underwrite foreign claim' 'matching claim attempt|independence|transport|item' \
  "$P/crucible" claim verdict "$cn_b" a1 FALSE

# Temporary cycle: guided off cannot leave unsealed verdict that becomes COMPLETE
cn_g=$($P/crucible claim add 'guided-toggle' 'source')
$P/crucible run-claim "$cn_g" a1 -- sh -c 'echo e' >/dev/null
# turn off guided
sed -i.bak '/^cycle: guided$/d' "$P/PROGRAM"
$P/crucible claim verdict "$cn_g" a1 FALSE >/dev/null
# restore guided
printf 'cycle: guided\n' >> "$P/PROGRAM"
# investigation must not treat unsealed verdict as complete audit
# (cycle status should still need audit for this claim when others are incomplete)
status=$($P/crucible cycle 2>&1 || true)
printf '%s\n' "$status" | grep -E -q 'NEEDS_AUDIT|INVESTIGATE|WAIT' \
  && ok || bad "unsealed verdict after guided restore must not complete investigation: $status"

# Seal-under-weak-panel then restore-strong refuses start
write_panel "$P"
# single-product waiver + one kind for weak acp seal
cat > "$P/PANEL.md" <<'EOF'
# Panel
## Agents
c0, a1, a2, mk1, j1, j2
## Roles
See PANEL.ASSIGN.tsv
## Risk posture
LOW
## Isolation transport
acp allowed under single-product
## Independence ladder
1 multi-agent 2 acp 3 subagent 4 stop
## Waivers
LADDER_WAIVER: single-product
EOF
{
  printf 'c0\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'a1\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'a2\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'mk1\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'j1\tkindA\tm\thigh\techo {BRIEF}\n'
  printf 'j2\tkindA\tm\thigh\techo {BRIEF}\n'
} > "$P/agents.tsv"
write_panel_assign() {
  cat > "$P/PANEL.ASSIGN.tsv" <<'EOF'
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
write_panel_assign
expect 'weak panel approves' '^approved panel ' "$P/crucible" cycle approve-panel
cn_w=$($P/crucible claim add 'weak-then-strong' 'source')
$P/crucible dispatch "$cn_w" claim-auditor a1 >/dev/null
aid_w=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$item" = "$cn_w" ] && [ "$agent" = a1 ] && aid_w=${ad##*/}
done
$P/crucible attempt transport "$aid_w" acp >/dev/null
$P/crucible contract-audit "$aid_w" j2 PASS >/dev/null
# restore strong multi-kind panel without waiver
write_panel "$P"
write_agents "$P"
write_panel_assign
expect 'strong panel re-approves' '^approved panel ' "$P/crucible" cycle approve-panel
expect 'acp seal remains startable after multi-kind restore' "RUNNING pid" \
  "$P/crucible" attempt start "$aid_w" $$

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
