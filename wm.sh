#!/bin/sh
# working-mode kernel — CHECK refusals. POSIX sh. Opt-in; not the guided runner.
set -eu

die() { printf 'refused: %s\n' "$*" >&2; exit 1; }
say() { printf '%s\n' "$*"; }

WM=".wm"
SPEC_ERR=

ensure_wm() {
  mkdir -p "$WM/bin" "$WM/evidence" "$WM/verdicts" "$WM/briefs" \
    "$WM/return" "$WM/return/history" "$WM/spawn" "$WM/history" \
    "$WM/receipts" "$WM/invoke"
}

file_sha256() {
  _f=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$_f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$_f" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$_f" | awk '{print $NF}'
  else
    die "no sha256 tool on PATH"
  fi
}

file_mtime() {
  if [ ! -e "$1" ]; then
    printf '0\n'
    return
  fi
  case $(uname -s) in
    Darwin) stat -f %m "$1" ;;
    *) stat -c %Y "$1" ;;
  esac
}

iso_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

workid_short() {
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    git rev-parse --short=12 HEAD
  else
    printf 'NOCOMMIT\n'
  fi
}

cmd_workid() { workid_short; }

kv_get() {
  _kg_file=$1
  _kg_key=$2
  [ -f "$_kg_file" ] || return 0
  awk -v k="$_kg_key" 'index($0, k ": ")==1 { print substr($0, length(k)+3); exit }' "$_kg_file"
}

panel_file() { printf '%s/PANEL.tsv' "$WM"; }

panel_field() {
  _pf_role=$1
  _pf_col=$2
  [ -f "$(panel_file)" ] || return 0
  awk -F '\t' -v r="$_pf_role" -v c="$_pf_col" '$1==r { print $c; exit }' "$(panel_file)"
}

panel_agent() { panel_field "$1" 2; }
panel_kind() { panel_field "$1" 3; }
panel_cmd() { panel_field "$1" 4; }

panel_set() {
  _ps_role=$1
  _ps_agent=$2
  _ps_kind=$3
  _ps_cmd=$4
  ensure_wm
  _ps_tmp="$WM/.PANEL.tsv.$$"
  _ps_found=0
  if [ -f "$(panel_file)" ]; then
    while IFS= read -r _ps_line || [ -n "$_ps_line" ]; do
      _ps_r=$(printf '%s\n' "$_ps_line" | awk -F '\t' '{print $1}')
      if [ "$_ps_r" = "$_ps_role" ]; then
        printf '%s\t%s\t%s\t%s\n' "$_ps_role" "$_ps_agent" "$_ps_kind" "$_ps_cmd"
        _ps_found=1
      else
        printf '%s\n' "$_ps_line"
      fi
    done < "$(panel_file)" > "$_ps_tmp"
  else
    : > "$_ps_tmp"
  fi
  if [ "$_ps_found" -eq 0 ]; then
    printf '%s\t%s\t%s\t%s\n' "$_ps_role" "$_ps_agent" "$_ps_kind" "$_ps_cmd" >> "$_ps_tmp"
  fi
  mv "$_ps_tmp" "$(panel_file)"
}

section_body() {
  _sb_h=$1
  _sb_file=$2
  awk -v h="$_sb_h" '
    $0 == h { p=1; next }
    /^## / { p=0 }
    p { print }
  ' "$_sb_file"
}

section_nonempty() {
  section_body "$1" "$2" | awk 'NF { found=1 } END { exit found ? 0 : 1 }'
}

section_first() {
  section_body "$1" "$2" | awk 'NF { print; exit }'
}

falsifier_slot() {
  [ -f SPEC.md ] || { printf '\n'; return; }
  _fs=$(section_body '## Focused falsifier' SPEC.md | awk 'NF { gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print }')
  printf '%s\n' "$_fs"
}

owned_paths() {
  _op_file=${1:-SPEC.md}
  [ -f "$_op_file" ] || return 0
  section_body '## Owned files' "$_op_file" | awk '
    /^- / {
      sub(/^- /, "")
      gsub(/^[[:space:]]+|[[:space:]]+$/, "")
      if ($0 != "" && $0 != "(none)") print
    }
  '
}

path_in_list() {
  _pil_path=$1
  _pil_list=$2
  [ -n "$_pil_list" ] || return 1
  printf '%s\n' "$_pil_list" | grep -qxF "$_pil_path"
}

porcelain_paths() {
  git status --porcelain=v1 -uall 2>/dev/null | awk '
    {
      rest = substr($0, 4)
      if (rest ~ / -> /) {
        sub(/.* -> /, "", rest)
      }
      print rest
    }
  '
}

owned_porcelain_dirty() {
  _opd_owned=$(owned_paths)
  [ -n "$_opd_owned" ] || return 1
  _opd_paths=$(porcelain_paths)
  [ -n "$_opd_paths" ] || return 1
  _opd_p=
  for _opd_p in $_opd_paths; do
    if path_in_list "$_opd_p" "$_opd_owned"; then
      return 0
    fi
  done
  return 1
}

product_commit_changed() {
  _pcc_pre=$1
  [ -n "$_pcc_pre" ] || return 1
  git rev-parse --verify "$_pcc_pre" >/dev/null 2>&1 || return 1
  _pcc_owned=$(owned_paths)
  [ -n "$_pcc_owned" ] || return 1
  _pcc_diff=$(git diff --name-only "$_pcc_pre"..HEAD 2>/dev/null || true)
  [ -n "$_pcc_diff" ] || return 1
  _pcc_p=
  for _pcc_p in $_pcc_diff; do
    if path_in_list "$_pcc_p" "$_pcc_owned"; then
      return 0
    fi
  done
  return 1
}

spec_ok() {
  SPEC_ERR=
  [ -f SPEC.md ] || { SPEC_ERR="SPEC.md missing"; return 1; }
  for _h in '## Goal' '## Non-goals' '## Owned files' '## Test files' '## Acceptance criteria' '## Focused falsifier' '## Stop conditions' '## Risk'; do
    grep -qF "$_h" SPEC.md || { SPEC_ERR="SPEC.md missing $_h"; return 1; }
    section_nonempty "$_h" SPEC.md || { SPEC_ERR="SPEC.md empty $_h"; return 1; }
  done
  _slot=$(falsifier_slot)
  _slot_n=$(printf '%s\n' "$_slot" | awk 'NF{c++} END{print c+0}')
  if [ "$_slot_n" -ne 1 ]; then
    SPEC_ERR="SPEC must not author the falsifier command"
    return 1
  fi
  _slot_t=$(printf '%s\n' "$_slot" | awk 'NF{print; exit}')
  if [ "$_slot_t" = "TEMPLATE-FALSIFIER-UNWRITTEN" ]; then
    SPEC_ERR="focused falsifier is not written"
    return 1
  fi
  if [ "$_slot_t" != "MAKER-WRITES" ]; then
    SPEC_ERR="SPEC must not author the falsifier command"
    return 1
  fi
  _own=$(owned_paths)
  [ -n "$_own" ] || { SPEC_ERR="owned files must be a non-empty list"; return 1; }
  _risk=$(section_first '## Risk' SPEC.md)
  case $_risk in
    LOW|MEDIUM|HIGH) ;;
    *) SPEC_ERR="risk must be LOW, MEDIUM, or HIGH"; return 1 ;;
  esac
  return 0
}

# Job-control `&` is background. `2>&1` / `&&` are not. Reap is not a substitute.
command_backgrounds() {
  _cb_scan=$(printf '%s\n' "$1" | sed -e 's/&&/ ANDAND /g' -e 's/[0-9][0-9]*>&[0-9][0-9]*//g' -e 's/>&[^[:space:]]*//g' -e 's/<&[^[:space:]]*//g' -e 's/&>[^[:space:]]*//g')
  case $_cb_scan in
    *'&'*) return 0 ;;
  esac
  return 1
}

guard_command() {
  _gc_cmd=$1
  _gc_role=${2:-}
  case $_gc_cmd in
    *'git push'*|*'git'*push*)
      case $_gc_cmd in
        *main*|*master*)
          die "push-main"
          ;;
      esac
      ;;
  esac
  case $_gc_cmd in
    *nohup*|*setsid*|*background:\ true*)
      die "background spawn refused"
      ;;
  esac
  if command_backgrounds "$_gc_cmd"; then
    die "background spawn refused"
  fi
  case $_gc_cmd in
    *'rm -rf'*|*'sudo '*)
      die "stop-ask: destructive"
      ;;
  esac
  case $_gc_cmd in
    *'--password'*|*'--token'*|*'--api-key'*)
      die "stop-ask: live write"
      ;;
  esac
  case $_gc_cmd in
    *'curl '*|*'wget '*|*'jira '*)
      # Research pass: specifier may curl/wget without live secret flags.
      if [ "$_gc_role" = specifier ] && [ ! -f RESEARCH.md ]; then
        case $_gc_cmd in
          *'jira '*)
            die "stop-ask: live write"
            ;;
        esac
      else
        die "stop-ask: live write"
      fi
      ;;
  esac
}

honest_isolation() { printf 'SUBAGENT-ISOLATED\n'; }

write_last_maker_run() {
  ensure_wm
  printf 'id: %s\nrole: %s\nwhen: %s\n' "$1" "$2" "$3" > "$WM/last-maker-run"
}

write_invoke_log() {
  # Parent of `wm run reviewer|scout` at exec. Leftover logs from before last maker-* are ignored.
  _wi_role=$1
  _wi_agent=$2
  _wi_cmd=$3
  _wi_pid=$4
  ensure_wm
  mkdir -p "$WM/invoke"
  _wi_am=
  if [ -f "$WM/last-maker-run" ]; then
    _wi_am=$(kv_get "$WM/last-maker-run" id)
  fi
  printf 'role: %s\nagent: %s\ncommand: %s\npid: %s\nwhen: %s\nwriter: wm-run\nafter-maker: %s\nISOLATION: SUBAGENT-ISOLATED\n' \
    "$_wi_role" "$_wi_agent" "$_wi_cmd" "$_wi_pid" "$(date +%s)" "$_wi_am" \
    > "$WM/invoke/${_wi_role}.log"
}

reviewer_exec_after_maker() {
  _re_role=$1
  _re_log="$WM/invoke/${_re_role}.log"
  [ -f "$_re_log" ] || return 1
  [ "$(kv_get "$_re_log" writer)" = wm-run ] || return 1
  [ "$(kv_get "$_re_log" role)" = "$_re_role" ] || return 1
  [ -f "$WM/last-maker-run" ] || return 1
  _re_need=$(kv_get "$WM/last-maker-run" id)
  _re_got=$(kv_get "$_re_log" after-maker)
  [ -n "$_re_need" ] && [ "$_re_got" = "$_re_need" ] || return 1
  return 0
}

snapshot_judge_artifacts() {
  _sj_out=$1
  : > "$_sj_out"
  _sj_rev=$(panel_agent reviewer)
  _sj_scout=$(panel_agent scout)
  _sj_p=
  for _sj_p in \
    "$WM/spawn/${_sj_rev}.stamp" "$WM/return/${_sj_rev}.md" "$WM/verdicts/${_sj_rev}.md" \
    "$WM/spawn/${_sj_scout}.stamp" "$WM/return/${_sj_scout}.md" "$WM/verdicts/${_sj_scout}.md" \
    "$WM/invoke/reviewer.log" "$WM/invoke/scout.log" "$WM/invoke.log" \
    "$WM/CLOSED"
  do
    [ -n "$_sj_p" ] || continue
    [ -f "$_sj_p" ] || continue
    printf '%s %s\n' "$_sj_p" "$(file_sha256 "$_sj_p")" >> "$_sj_out"
  done
  for _sj_p in "$WM/briefs"/reviewer.* "$WM/briefs"/scout.*; do
    [ -f "$_sj_p" ] || continue
    printf '%s %s\n' "$_sj_p" "$(file_sha256 "$_sj_p")" >> "$_sj_out"
  done
}

judge_artifacts_changed() {
  _jc_snap=$1
  _jc_now="$WM/.judge.now.$$"
  snapshot_judge_artifacts "$_jc_now"
  if [ ! -s "$_jc_now" ]; then
    rm -f "$_jc_now"
    return 1
  fi
  while IFS= read -r _jc_line || [ -n "$_jc_line" ]; do
    [ -n "$_jc_line" ] || continue
    _jc_path=${_jc_line%% *}
    _jc_hash=${_jc_line#* }
    _jc_old=
    if [ -f "$_jc_snap" ]; then
      _jc_old=$(awk -v p="$_jc_path" '$1==p { print $2; exit }' "$_jc_snap")
    fi
    if [ "$_jc_old" != "$_jc_hash" ]; then
      rm -f "$_jc_now"
      return 0
    fi
  done < "$_jc_now"
  rm -f "$_jc_now"
  return 1
}

role_brief_exists() {
  _rb_role=$1
  for _rb_f in "$WM/briefs/${_rb_role}".*; do
    [ -f "$_rb_f" ] || return 1
    return 0
  done
  return 1
}

post_spawn_return_ok() {
  _ps_agent=$1
  _ps_word=$2
  _ps_role=$3
  _ps_st="$WM/spawn/${_ps_agent}.stamp"
  _ps_ret="$WM/return/${_ps_agent}.md"
  [ -f "$_ps_st" ] || return 1
  grep -q 'absent_return: yes' "$_ps_st" || return 1
  _ps_strole=$(kv_get "$_ps_st" role)
  [ "$_ps_strole" = "$_ps_role" ] || return 1
  [ -f "$_ps_ret" ] || return 1
  _ps_t=$(kv_get "$_ps_st" t)
  _ps_mt=$(file_mtime "$_ps_ret")
  if [ -n "$_ps_t" ] && [ "$_ps_mt" -lt "$_ps_t" ]; then
    return 1
  fi
  _ps_rw=$(kv_get "$_ps_ret" WORD)
  [ "$_ps_rw" = "$_ps_word" ] || return 1
  return 0
}

verdict_is_closeable() {
  _vc_file=$1
  [ -f "$_vc_file" ] || return 1
  _vc_word=$(kv_get "$_vc_file" VERDICT)
  _vc_who=$(kv_get "$_vc_file" AGENT)
  _vc_ev=$(kv_get "$_vc_file" EVIDENCE)
  [ -n "$_vc_who" ] || return 1
  case $_vc_who in
    parent|coordinator|loop) return 1 ;;
  esac
  _vc_maker=$(panel_agent maker)
  _vc_rev=$(panel_agent reviewer)
  [ "$_vc_who" != "$_vc_maker" ] || return 1
  [ -n "$_vc_ev" ] && [ -f "$_vc_ev" ] || return 1
  grep -q '^working-mode-run/1$' "$_vc_ev" || return 1
  grep -q "^agent: ${_vc_who}$" "$_vc_ev" || return 1
  grep -q "^work-id: " "$_vc_ev" || return 1
  _vc_drole=$(kv_get "$_vc_ev" dispatch-role)
  _vc_dagent=$(kv_get "$_vc_ev" dispatch-agent)
  [ "$_vc_dagent" != "$_vc_maker" ] || return 1
  _vc_runid_v=$(kv_get "$_vc_file" RUN-ID)
  _vc_runid_s=
  if [ -f "$WM/spawn/${_vc_who}.stamp" ]; then
    _vc_runid_s=$(kv_get "$WM/spawn/${_vc_who}.stamp" run-id)
  fi
  [ -n "$_vc_runid_v" ] && [ -n "$_vc_runid_s" ] && [ "$_vc_runid_v" = "$_vc_runid_s" ] || return 1
  case $_vc_word in
    PASS)
      [ "$_vc_who" = "$_vc_rev" ] || return 1
      [ "$_vc_drole" = reviewer ] || return 1
      [ "$_vc_dagent" = "$_vc_rev" ] || return 1
      post_spawn_return_ok "$_vc_who" PASS reviewer || return 1
      role_brief_exists reviewer || return 1
      reviewer_exec_after_maker reviewer || return 1
      [ -f "$WM/green.status" ] && [ "$(cat "$WM/green.status")" = ok ] || return 1
      [ -f "$WM/FALSIFIER" ] || return 1
      _vc_fals=$(awk 'NF{print; exit}' "$WM/FALSIFIER")
      [ -n "$_vc_fals" ] || return 1
      grep -F "$_vc_fals" "$_vc_ev" >/dev/null || return 1
      grep -q '^exit: 0$' "$_vc_ev" || return 1
      ;;
    NO-BUILD)
      [ "$_vc_who" = "$_vc_rev" ] || return 1
      [ "$_vc_drole" = reviewer ] || return 1
      [ "$_vc_dagent" = "$_vc_who" ] || return 1
      post_spawn_return_ok "$_vc_who" NO-BUILD reviewer || return 1
      role_brief_exists reviewer || return 1
      reviewer_exec_after_maker reviewer || return 1
      if owned_porcelain_dirty; then
        return 1
      fi
      if [ -f "$WM/pre-falsify-wid" ] && product_commit_changed "$(cat "$WM/pre-falsify-wid")"; then
        return 1
      fi
      ;;
    *)
      return 1
      ;;
  esac
  return 0
}

live_is_pass_or_nobuild() {
  for _ln in "$WM/verdicts"/*.md; do
    [ -f "$_ln" ] || continue
    _ln_v=$(kv_get "$_ln" VERDICT)
    if [ "$_ln_v" = PASS ] || [ "$_ln_v" = NO-BUILD ]; then
      if verdict_is_closeable "$_ln"; then
        return 0
      fi
    fi
  done
  return 1
}

# CLOSED file is not success by itself. Honor only after close() / 5c.
closed_is_closeable() {
  [ -f "$WM/CLOSED" ] || return 1
  live_is_pass_or_nobuild || return 1
  return 0
}

panel_valid() {
  _pv_m=$(panel_agent maker)
  _pv_r=$(panel_agent reviewer)
  [ -n "$_pv_m" ] && [ "$_pv_m" != - ] || return 1
  [ -n "$_pv_r" ] && [ "$_pv_r" != - ] || return 1
  [ "$_pv_m" != "$_pv_r" ] || return 1
  case $_pv_m in parent|coordinator|loop) return 1 ;; esac
  case $_pv_r in parent|coordinator|loop) return 1 ;; esac
  return 0
}

spec_committed() {
  git ls-files --error-unmatch SPEC.md >/dev/null 2>&1 || return 1
  git diff --quiet HEAD -- SPEC.md 2>/dev/null || return 1
  git diff --cached --quiet -- SPEC.md 2>/dev/null || return 1
  return 0
}

head_eq_ref() {
  _he_ref=$1
  [ -n "$_he_ref" ] || return 1
  _he_h=$(git rev-parse HEAD)
  _he_p=$(git rev-parse "$_he_ref" 2>/dev/null) || return 1
  [ "$_he_h" = "$_he_p" ]
}

read_falsifier_cmd() {
  [ -f "$WM/FALSIFIER" ] || die "FALSIFIER missing"
  _rf_n=$(awk 'NF{c++} END{print c+0}' "$WM/FALSIFIER")
  [ "$_rf_n" -eq 1 ] || die "FALSIFIER must be exactly one line"
  _rf=$(awk 'NF{print; exit}' "$WM/FALSIFIER")
  [ -n "$_rf" ] || die "FALSIFIER empty"
  case $_rf in
    *TEMPLATE-FALSIFIER-UNWRITTEN*) die "FALSIFIER is template" ;;
  esac
  printf '%s\n' "$_rf"
}

meta_agent() { kv_get "$WM/FALSIFIER.meta" agent; }

check_falsifier_meta() {
  [ -f "$WM/FALSIFIER.meta" ] || die "FALSIFIER.meta missing"
  _cfm=$(meta_agent)
  [ -n "$_cfm" ] || die "coordinator-authored meta"
  case $_cfm in
    parent|coordinator|loop|'') die "coordinator-authored meta" ;;
  esac
  _cfm_h=$(kv_get "$WM/FALSIFIER.meta" sha256)
  _cfm_got=$(file_sha256 "$WM/FALSIFIER")
  if [ -n "$_cfm_h" ] && [ "$_cfm_h" != "$_cfm_got" ]; then
    die "hash mismatch"
  fi
}

# Cycle logbook. Adopted programs: .crucible/<prog>/LESSONS.md.
# Kernel fixtures without adopt: repo-root LESSONS.md (not $HOME, not .wm).
lessons_file() {
  if [ -n "${WM_PROGRAM:-}" ] && [ -d ".crucible/${WM_PROGRAM}" ]; then
    printf '%s\n' ".crucible/${WM_PROGRAM}/LESSONS.md"
    return 0
  fi
  if [ -d .crucible ]; then
    for _lf_p in .crucible/*/PROGRAM; do
      [ -f "$_lf_p" ] || continue
      printf '%s/LESSONS.md\n' "$(dirname "$_lf_p")"
      return 0
    done
  fi
  printf '%s\n' "LESSONS.md"
}

arch_fence_active() {
  _af_f=$(lessons_file)
  [ -f "$_af_f" ] || return 1
  awk '/^ARCH:/ { found=1 } END { exit found ? 0 : 1 }' "$_af_f"
}

# RULE 23 analogue: maker briefs include LESSONS (or NONE). ARCH: lines are
# not maker-binding bullets (12e); they STOP-ASK instead of starting a maker.
emit_lessons_section() {
  printf '\n## Lessons from earlier items — these bind you\n\n'
  _el_f=$(lessons_file)
  if [ ! -s "$_el_f" ]; then
    printf 'NONE\n'
    return 0
  fi
  _el_any=0
  while IFS= read -r _el_line || [ -n "$_el_line" ]; do
    case $_el_line in
      ARCH:*) continue ;;
    esac
    printf '%s\n' "$_el_line"
    _el_any=1
  done < "$_el_f"
  if [ "$_el_any" -eq 0 ]; then
    printf 'NONE\n'
  fi
}

resolve_loop_lesson() {
  _rl=${WM_LESSON:-}
  if [ -z "$_rl" ] && [ -f "$WM/lesson" ]; then
    _rl=$(awk 'NF { print; exit }' "$WM/lesson")
  fi
  if [ -z "$_rl" ]; then
    _rl=NONE
  fi
  printf '%s\n' "$_rl"
}

research_skill_present() {
  [ -f .crucible/skills/research/SKILL.md ] || [ -f skills/research/SKILL.md ]
}

write_brief() {
  _wb_role=$1
  _wb_agent=$2
  _wb_wid=$(workid_short)
  ensure_wm
  _wb_path="$WM/briefs/${_wb_role}.${_wb_wid}.md"
  {
    printf 'Read this file and follow it exactly.\n'
    printf 'role: %s\nagent: %s\n' "$_wb_role" "$_wb_agent"
    case $_wb_role in
      maker-falsify)
        printf 'Write .wm/FALSIFIER (one command) and .wm/FALSIFIER.meta. Commit. Do not implement product owned files. Do not write verdicts.\n'
        emit_lessons_section
        ;;
      maker-build)
        printf 'Implement owned files. Commit. Do not write verdicts.\n'
        emit_lessons_section
        ;;
      reviewer)
        printf 'Write .wm/return/%s.md with WORD: and EVIDENCE:. Re-run the named falsifier via .wm/bin/wm evidence. If .wm/red.status is no-build, WORD must be NO-BUILD not PASS. Do not use maker rationale.\n' "$_wb_agent"
        ;;
      specifier)
        if research_skill_present && [ ! -f RESEARCH.md ]; then
          printf 'Read IDEA.md. Write RESEARCH.md only (stack survey, constraints, non-goals, competitors if known). Do not write SPEC.md or MAP.md yet. Do not implement product. Do not write MAP-ACCEPT, CLOSED PASS, or FALSIFIER. Do not use live tokens or passwords.\n'
        else
          printf 'Read IDEA.md. Read the architecture SKILL.md if present. If the idea is underspecified, write QUESTIONS.md (at most 7 questions, one topic each) and stop. Do not invent answers. Do not implement product.\n'
          printf 'When specified, write SPEC.md (required headings, MAKER-WRITES, owned files, LOW|MEDIUM|HIGH), architecture/modules.md TSV, and MAP.md. MAPPER is this agent (%s). Do not write MAP-ACCEPT.\n' "$_wb_agent"
        fi
        ;;
      scout)
        _wb_mw=$(map_word_recorded)
        if [ -f MAP.md ] && { [ -z "$_wb_mw" ] || [ "$_wb_mw" = MAP-REVISE ]; }; then
          printf 'Write .wm/return/%s.md with WORD: MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK (not authored by the mapper) plus MAP: MAP.md.\n' "$_wb_agent"
        else
          printf 'Write .wm/return/%s.md with WORD: NO-BUILD if the capability already exists, plus EVIDENCE:.\n' "$_wb_agent"
        fi
        ;;
    esac
  } > "$_wb_path"
  printf '%s\n' "$_wb_path"
}

cmd_init() {
  ensure_wm
  _in_src=${WM_ENGINE:-}
  if [ -z "$_in_src" ]; then
    _in_src=$0
  fi
  case $_in_src in
    /*) ;;
    *) _in_src=$(CDPATH= cd "$(dirname "$_in_src")" && pwd)/$(basename "$_in_src") ;;
  esac
  [ -f "$_in_src" ] || die "WM_ENGINE not found: $_in_src"
  if [ "$_in_src" != "$PWD/$WM/bin/wm" ]; then
    cp "$_in_src" "$WM/bin/wm"
  fi
  chmod +x "$WM/bin/wm"
  printf 'engine: %s\n' "$_in_src" > "$WM/ENGINE"
  if [ ! -f "$(panel_file)" ]; then
    panel_set coordinator parent grok -
  fi
  if [ ! -d .git ]; then
    git init -q
  fi
  _in_em=$(git config user.email 2>/dev/null || true)
  if [ -z "$_in_em" ]; then
    git config user.email 'wm@local'
    git config user.name 'working-mode'
  fi
  if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
    git add -A
    git commit -qm 'wm: init'
  fi
  say "initialized $WM"
}

cmd_cast() {
  _ca_role=${1:-}
  _ca_agent=${2:-}
  _ca_kind=${3:-}
  shift 3 2>/dev/null || die "usage: wm cast ROLE AGENT KIND COMMAND"
  _ca_cmd=$*
  [ -n "$_ca_cmd" ] || _ca_cmd=
  case $_ca_role in
    coordinator|specifier|maker|reviewer|scout) ;;
    *) die "unknown role: $_ca_role" ;;
  esac
  [ -n "$_ca_agent" ] && [ "$_ca_agent" != - ] || die "agent id required"
  [ -n "$_ca_kind" ] || die "kind required"
  case $_ca_role in
    maker|reviewer)
      case $_ca_agent in
        parent|coordinator|loop) die "maker or reviewer cannot be parent/coordinator/loop" ;;
      esac
      if [ -z "$_ca_cmd" ] || [ "$_ca_cmd" = - ]; then
        die "INDEPENDENCE_UNAVAILABLE: no CLI worker"
      fi
      ;;
    specifier)
      case $_ca_agent in
        parent|coordinator|loop) die "specifier cannot be parent/coordinator/loop" ;;
      esac
      ;;
  esac
  ensure_wm
  if [ "$_ca_role" = maker ]; then
    _ca_other=$(panel_agent reviewer)
    [ "$_ca_other" != "$_ca_agent" ] || die "maker and reviewer must be distinct agents (both $_ca_agent)"
  fi
  if [ "$_ca_role" = reviewer ]; then
    _ca_other=$(panel_agent maker)
    [ "$_ca_other" != "$_ca_agent" ] || die "maker and reviewer must be distinct agents (both $_ca_agent)"
  fi
  if [ "$_ca_role" = maker ]; then
    refuse_if_mapper_is_maker "$_ca_agent"
    refuse_if_specifier_is_maker "$_ca_agent"
  fi
  if [ "$_ca_role" = specifier ]; then
    _ca_maker=$(panel_agent maker)
    if [ -n "$_ca_maker" ] && [ "$_ca_maker" != - ] && [ "$_ca_maker" = "$_ca_agent" ]; then
      die "specifier cannot be maker ($_ca_agent)"
    fi
  fi
  panel_set "$_ca_role" "$_ca_agent" "$_ca_kind" "$_ca_cmd"
  say "cast $_ca_role=$_ca_agent kind=$_ca_kind"
}

check_routing_batteries() {
  _rt=
  if [ -f ROUTING.tsv ]; then
    _rt=ROUTING.tsv
  elif [ -f .crucible/ROUTING.tsv ]; then
    _rt=.crucible/ROUTING.tsv
  else
    for _cand in .crucible/*/ROUTING.tsv; do
      [ -f "$_cand" ] || continue
      _rt=$_cand
      break
    done
  fi
  [ -n "$_rt" ] || return 0
  _miss=
  while IFS="$(printf '\t')" read -r _ph _job _bat _role _stake _req _rest; do
    [ -n "${_ph:-}" ] || continue
    case $_ph in
      phase|\#*) continue ;;
    esac
    [ "${_req:-}" = yes ] || continue
    [ -n "${_bat:-}" ] && [ "$_bat" != - ] || continue
    if [ ! -f ".crucible/skills/${_bat}/SKILL.md" ]; then
      _miss="$_miss $_bat"
    fi
  done < "$_rt"
  [ -z "$_miss" ] || die "required battery missing:$_miss"
}

mapper_id() { kv_get "$WM/mapper" id; }

refuse_if_mapper_is_maker() {
  _rmm_agent=$1
  _rmm_mapper=$(mapper_id)
  [ -n "$_rmm_mapper" ] || return 0
  [ "$_rmm_agent" != "$_rmm_mapper" ] || die "mapper cannot be maker ($_rmm_agent)"
}

refuse_if_specifier_is_maker() {
  _rsm_agent=$1
  _rsm_spec=$(panel_agent specifier)
  [ -n "$_rsm_spec" ] && [ "$_rsm_spec" != - ] || return 0
  [ "$_rsm_agent" != "$_rsm_spec" ] || die "specifier cannot be maker ($_rsm_agent)"
}

role_has_cli() {
  _rhc_agent=$(panel_agent "$1")
  [ -n "$_rhc_agent" ] && [ "$_rhc_agent" != - ] || return 1
  _rhc_cmd=$(panel_cmd "$1")
  [ -n "$_rhc_cmd" ] && [ "$_rhc_cmd" != - ]
}

# TSV only (module_id, root_path, …). root: lines and markdown tables do not name roots.
list_module_roots() {
  [ -f architecture/modules.md ] || return 1
  awk -F '\t' '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    /^#/ { next }
    /^[[:space:]]*$/ { next }
    NF < 2 { next }
    trim($1) == "module_id" { next }
    {
      r = trim($2)
      if (r != "") print r
    }
  ' architecture/modules.md
}

path_under_root() {
  case $1 in
    "$2"|"$2"/*) return 0 ;;
  esac
  return 1
}

path_fits_modules() {
  _pfm_path=$1
  _pfm_roots=$2
  case $_pfm_path in
    ''|*..*|/*) return 1 ;;
  esac
  _pfm_r=
  while IFS= read -r _pfm_r || [ -n "$_pfm_r" ]; do
    [ -n "$_pfm_r" ] || continue
    if path_under_root "$_pfm_path" "$_pfm_r"; then
      return 0
    fi
  done <<EOF
$_pfm_roots
EOF
  return 1
}

emit_map_owned() {
  [ -f MAP.md ] || return 0
  awk -F '\t' '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    {
      if (index($0, "\t") == 0) next
      if (!oc) {
        for (i = 1; i <= NF; i++) {
          c = trim($i)
          if (c == "owned_paths" || c == "owned") oc = i
        }
        if (oc) next
        next
      }
      if (oc && NF >= oc) {
        c = trim($oc)
        if (c != "" && c != "-" && c != "owned_paths") print c
      }
    }
  ' MAP.md
}

# MAP.md TSV rows: id, module, owned_paths, depends_on, risk (no status).
emit_map_rows() {
  [ -f MAP.md ] || return 0
  awk -F '\t' '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    {
      if (index($0, "\t") == 0) next
      if (!hid) {
        for (i = 1; i <= NF; i++) {
          c = trim($i)
          if (c == "id") hid = i
          if (c == "module") hmod = i
          if (c == "owned_paths" || c == "owned") hown = i
          if (c == "depends_on" || c == "depends-on") hdep = i
          if (c == "risk") hrisk = i
        }
        if (hid && hmod && hown && hrisk) next
        hid = hmod = hown = hdep = hrisk = 0
        next
      }
      id = (hid <= NF) ? trim($hid) : ""
      mod = (hmod <= NF) ? trim($hmod) : ""
      own = (hown <= NF) ? trim($hown) : ""
      dep = (hdep && hdep <= NF) ? trim($hdep) : "-"
      risk = (hrisk <= NF) ? trim($hrisk) : ""
      if (id == "" || id == "id") next
      if (dep == "") dep = "-"
      printf "%s\t%s\t%s\t%s\t%s\n", id, mod, own, dep, risk
    }
  ' MAP.md
}

module_live_write() {
  _mlw_mod=$1
  [ -n "$_mlw_mod" ] || { printf 'no\n'; return 0; }
  [ -f architecture/modules.md ] || { printf 'no\n'; return 0; }
  awk -F '\t' -v m="$_mlw_mod" '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    /^#/ { next }
    /^[[:space:]]*$/ { next }
    NF < 2 { next }
    trim($1) == "module_id" { next }
    trim($1) == m {
      if (NF >= 6) print trim($6)
      else print "no"
      exit
    }
  ' architecture/modules.md
}

map_word_recorded() { kv_get "$WM/map-verdict" WORD; }

human_sign_present() {
  [ -f MAP-HUMAN ] || return 1
  _hs_who=$(kv_get MAP-HUMAN SIGNED)
  _hs_who=$(printf '%s\n' "$_hs_who" | awk '{ gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print }')
  [ -n "$_hs_who" ] || return 1
  case $_hs_who in
    parent|coordinator|loop|mapper|maker|reviewer|-|'') return 1 ;;
  esac
  _hs_mapper=$(mapper_id)
  if [ -n "$_hs_mapper" ] && [ "$_hs_who" = "$_hs_mapper" ]; then
    return 1
  fi
  _hs_maker=$(panel_agent maker)
  if [ -n "$_hs_maker" ] && [ "$_hs_maker" != - ] && [ "$_hs_who" = "$_hs_maker" ]; then
    return 1
  fi
  _hs_rev=$(panel_agent reviewer)
  if [ -n "$_hs_rev" ] && [ "$_hs_rev" != - ] && [ "$_hs_who" = "$_hs_rev" ]; then
    return 1
  fi
  _hs_map=$(kv_get MAP-HUMAN MAP)
  _hs_map=$(printf '%s\n' "$_hs_map" | awk '{ gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print }')
  [ -n "$_hs_map" ] || return 1
  [ -f "$_hs_map" ] || return 1
  _hs_want=$(kv_get MAP-HUMAN SHA256)
  if [ -z "$_hs_want" ]; then
    _hs_want=$(kv_get MAP-HUMAN MAP-SHA256)
  fi
  _hs_want=$(printf '%s\n' "$_hs_want" | awk '{ gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print tolower($0) }')
  [ -n "$_hs_want" ] || return 1
  _hs_got=$(file_sha256 "$_hs_map")
  _hs_got=$(printf '%s\n' "$_hs_got" | awk '{ print tolower($0) }')
  [ "$_hs_want" = "$_hs_got" ] || return 1
  return 0
}

high_kinds_ok() {
  _hk_mk=$(panel_kind maker)
  _hk_rk=$(panel_kind reviewer)
  [ -n "$_hk_mk" ] && [ "$_hk_mk" != - ] || return 1
  [ -n "$_hk_rk" ] && [ "$_hk_rk" != - ] || return 1
  [ "$_hk_mk" != "$_hk_rk" ]
}

slice_risk() {
  _sr_id=$1
  [ -n "$_sr_id" ] && [ -f slices.tsv ] || return 0
  awk -F '\t' -v id="$_sr_id" '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        c = trim($i)
        if (c == "id") hid = i
        if (c == "risk") hr = i
      }
      next
    }
    hid && hr && trim($hid) == id { print trim($hr); exit }
  ' slices.tsv
}

slice_module() {
  _smod_id=$1
  [ -n "$_smod_id" ] && [ -f slices.tsv ] || return 0
  awk -F '\t' -v id="$_smod_id" '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        c = trim($i)
        if (c == "id") hid = i
        if (c == "module") hm = i
      }
      next
    }
    hid && hm && trim($hid) == id { print trim($hm); exit }
  ' slices.tsv
}

slice_needs_human_sign() {
  _snhs_id=$1
  [ -n "$_snhs_id" ] || return 1
  _snhs_risk=$(slice_risk "$_snhs_id")
  if [ "$_snhs_risk" = HIGH ]; then
    return 0
  fi
  _snhs_mod=$(slice_module "$_snhs_id")
  _snhs_lw=$(module_live_write "$_snhs_mod")
  if [ "$_snhs_lw" = yes ]; then
    return 0
  fi
  return 1
}

map_needs_human_sign() {
  _nh_sid=$(first_ready_slice)
  if [ -z "$_nh_sid" ] && [ -f "$WM/slice-in-flight" ]; then
    _nh_sid=$(kv_get "$WM/slice-in-flight" id)
  fi
  if [ -n "$_nh_sid" ]; then
    slice_needs_human_sign "$_nh_sid"
    return $?
  fi
  _nh_rows=
  if [ -f slices.tsv ]; then
    _nh_rows=$(awk -F '\t' '
      function trim(s) {
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
        return s
      }
      NR == 1 {
        for (i = 1; i <= NF; i++) {
          c = trim($i)
          if (c == "module") hm = i
          if (c == "risk") hr = i
        }
        next
      }
      hm && hr { printf "%s\t%s\n", trim($hm), trim($hr) }
    ' slices.tsv)
  elif [ -f MAP.md ]; then
    _nh_rows=$(emit_map_rows | awk -F '\t' '{ printf "%s\t%s\n", $2, $5 }')
  else
    return 1
  fi
  [ -n "$_nh_rows" ] || return 1
  while IFS="$(printf '\t')" read -r _nh_mod _nh_risk || [ -n "${_nh_mod:-}" ]; do
    [ -n "${_nh_mod:-}${_nh_risk:-}" ] || continue
    if [ "$_nh_risk" = HIGH ]; then
      return 0
    fi
    _nh_lw=$(module_live_write "$_nh_mod")
    if [ "$_nh_lw" = yes ]; then
      return 0
    fi
  done <<EOF
$_nh_rows
EOF
  return 1
}

first_ready_slice() {
  [ -f slices.tsv ] || return 0
  awk -F '\t' '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    function deps_ready(dep,    n, a, i, t) {
      if (dep == "-" || dep == "") return 1
      n = split(dep, a, ",")
      for (i = 1; i <= n; i++) {
        t = trim(a[i])
        if (t == "" || t == "-") continue
        if (status[t] != "CLOSED") return 0
      }
      return 1
    }
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        c = trim($i)
        if (c == "id") hid = i
        if (c == "status") hs = i
        if (c == "depends_on" || c == "depends-on") hd = i
      }
      next
    }
    hid && hs {
      n++
      order[n] = trim($hid)
      status[order[n]] = trim($hs)
      deps[order[n]] = hd ? trim($hd) : "-"
    }
    END {
      for (i = 1; i <= n; i++) {
        id = order[i]
        if (status[id] == "READY" && deps_ready(deps[id])) {
          print id
          exit
        }
      }
    }
  ' slices.tsv
}

mark_slice_status() {
  _mss_id=$1
  _mss_st=$2
  [ -n "$_mss_id" ] && [ -n "$_mss_st" ] || return 0
  [ -f slices.tsv ] || return 0
  ensure_wm
  _mss_tmp="$WM/.slices.tsv.$$"
  awk -F '\t' -v id="$_mss_id" -v st="$_mss_st" 'BEGIN { OFS="\t" }
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        c = trim($i)
        if (c == "id") hid = i
        if (c == "status") hs = i
      }
      print
      next
    }
    hid && hs && trim($hid) == id { $hs = st }
    { print }
  ' slices.tsv > "$_mss_tmp"
  mv "$_mss_tmp" slices.tsv
}

# Clear one brick's receipts so the next READY slice can start. Same set as
# the 1.7.0 blank-home harness reset; slice-close is not work-close.
reset_brick() {
  ensure_wm
  rm -f "$WM/CLOSED" "$WM/FALSIFIER" "$WM/FALSIFIER.meta" "$WM/FALSIFIER.sha256" \
    "$WM/red.status" "$WM/red.out" "$WM/built.status" "$WM/built.reason" \
    "$WM/green.status" "$WM/green.out" "$WM/pre-falsify-wid" "$WM/pre-build-wid" \
    "$WM/last-maker-run" "$WM/reviewer-ran" "$WM/slice-in-flight" \
    "$WM/dispatch" "$WM/worker.out" "$WM/worker.err"
  rm -rf "$WM/verdicts" "$WM/return" "$WM/invoke" "$WM/spawn" "$WM/briefs" "$WM/evidence"
  mkdir -p "$WM/verdicts" "$WM/return" "$WM/return/history" "$WM/invoke" "$WM/spawn" \
    "$WM/briefs" "$WM/evidence"
}

materialize_slices() {
  _ms_status=$1
  [ -n "$_ms_status" ] || die "slice status required"
  [ -f MAP.md ] || die "MAP.md missing"
  ensure_wm
  _ms_tmp="$WM/.slices.tsv.$$"
  printf 'id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n' > "$_ms_tmp"
  _ms_n=0
  _ms_rows=$(emit_map_rows)
  while IFS="$(printf '\t')" read -r _ms_id _ms_mod _ms_own _ms_dep _ms_risk || [ -n "${_ms_id:-}" ]; do
    [ -n "${_ms_id:-}" ] || continue
    [ -n "$_ms_mod" ] || { rm -f "$_ms_tmp"; die "slice missing module: $_ms_id"; }
    [ -n "$_ms_own" ] && [ "$_ms_own" != - ] || { rm -f "$_ms_tmp"; die "slice missing owned_paths: $_ms_id"; }
    [ -n "$_ms_dep" ] || _ms_dep=-
    case $_ms_risk in
      LOW|HIGH) ;;
      *) rm -f "$_ms_tmp"; die "slice risk must be LOW or HIGH: $_ms_id" ;;
    esac
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$_ms_id" "$_ms_mod" "$_ms_own" "$_ms_dep" "$_ms_risk" "$_ms_status" >> "$_ms_tmp"
    _ms_n=$((_ms_n + 1))
  done <<EOF
$_ms_rows
EOF
  if [ "$_ms_n" -eq 0 ]; then
    rm -f "$_ms_tmp"
    die "MAP.md names no slices"
  fi
  mv "$_ms_tmp" slices.tsv
}

guard_map_before_maker() {
  if [ ! -f MAP.md ] && [ ! -f slices.tsv ]; then
    return 0
  fi
  _gm_word=$(map_word_recorded)
  [ "$_gm_word" = MAP-ACCEPT ] || die "map-verdict MAP-ACCEPT required before maker"
  if map_needs_human_sign; then
    human_sign_present || die "MAP-HUMAN required for HIGH/live"
  fi
  _gm_sid=$(first_ready_slice)
  _gm_risk=
  if [ -n "$_gm_sid" ]; then
    _gm_risk=$(slice_risk "$_gm_sid")
  fi
  if [ -z "$_gm_risk" ] && [ -f MAP.md ]; then
    _gm_risk=$(emit_map_rows | awk -F '\t' 'NF >= 5 { print $5; exit }')
  fi
  if [ "$_gm_risk" = HIGH ]; then
    high_kinds_ok || die "STOP-ASK: HIGH requires distinct maker and reviewer kinds"
  fi
}

split_csv_paths() {
  printf '%s\n' "$1" | awk -F ',' '{
    for (i = 1; i <= NF; i++) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", $i)
      if ($i != "") print $i
    }
  }'
}

stop_if_changes_architecture() {
  _sia_f=
  for _sia_f in architecture/modules.md MAP.md SPEC.md DESIGN.md "$@"; do
    [ -n "$_sia_f" ] || continue
    [ -f "$_sia_f" ] || continue
    if grep -F -q 'CHANGES-ARCHITECTURE' "$_sia_f"; then
      die "STOP: CHANGES-ARCHITECTURE (no silent second pattern)"
    fi
  done
}

cmd_record_mapper() {
  _rm_from=
  _rm_agent=
  if [ "${1:-}" = --from ]; then
    _rm_from=${2:-}
    [ -n "$_rm_from" ] || die "usage: wm record-mapper --from FILE"
    [ -f "$_rm_from" ] || die "architecture output missing: $_rm_from"
    _rm_agent=$(kv_get "$_rm_from" MAPPER)
    [ -n "$_rm_agent" ] || die "architecture output missing MAPPER"
  else
    _rm_agent=${1:-}
    [ -n "$_rm_agent" ] || die "usage: wm record-mapper AGENT | wm record-mapper --from FILE"
  fi
  case $_rm_agent in
    parent|coordinator|loop|-|'') die "mapper id required" ;;
  esac
  ensure_wm
  _rm_maker=$(panel_agent maker)
  if [ -n "$_rm_maker" ] && [ "$_rm_maker" != - ] && [ "$_rm_maker" = "$_rm_agent" ]; then
    die "mapper cannot be maker ($_rm_agent)"
  fi
  {
    printf 'id: %s\n' "$_rm_agent"
    if [ -n "$_rm_from" ]; then
      printf 'from: %s\n' "$_rm_from"
    fi
    printf 'when: %s\n' "$(iso_now)"
  } > "$WM/mapper"
  say "mapper $_rm_agent"
}

cmd_check_module_fit() {
  _cm_extra=
  _cm_spec=
  while [ $# -gt 0 ]; do
    case $1 in
      --path)
        shift
        [ -n "${1:-}" ] || die "usage: wm check-module-fit [--path PATH] [SPEC.md]"
        _cm_extra="${_cm_extra}
$1"
        shift
        ;;
      --)
        shift
        break
        ;;
      -*)
        die "usage: wm check-module-fit [--path PATH] [SPEC.md]"
        ;;
      *)
        _cm_spec=$1
        shift
        ;;
    esac
  done
  stop_if_changes_architecture "$_cm_spec"
  [ -f architecture/modules.md ] || die "architecture/modules.md missing"
  _cm_roots=$(list_module_roots) || _cm_roots=
  [ -n "$_cm_roots" ] || die "architecture/modules.md names no module roots"
  _cm_r=
  while IFS= read -r _cm_r || [ -n "$_cm_r" ]; do
    [ -n "$_cm_r" ] || continue
    case $_cm_r in
      *..*|/*) die "invalid module root: $_cm_r" ;;
    esac
    [ -d "$_cm_r" ] || die "module root does not exist: $_cm_r"
  done <<EOF
$_cm_roots
EOF
  ensure_wm
  _cm_list="$WM/.fit-paths.$$"
  : > "$_cm_list"
  if [ -n "$_cm_extra" ]; then
    printf '%s\n' "$_cm_extra" >> "$_cm_list"
  fi
  if [ -n "$_cm_spec" ]; then
    owned_paths "$_cm_spec" >> "$_cm_list"
  elif [ -f SPEC.md ]; then
    owned_paths >> "$_cm_list"
  fi
  while IFS= read -r _cm_cell || [ -n "$_cm_cell" ]; do
    [ -n "$_cm_cell" ] || continue
    split_csv_paths "$_cm_cell" >> "$_cm_list"
  done <<EOF
$(emit_map_owned)
EOF
  _cm_p=
  while IFS= read -r _cm_p || [ -n "$_cm_p" ]; do
    [ -n "$_cm_p" ] || continue
    if ! path_fits_modules "$_cm_p" "$_cm_roots"; then
      rm -f "$_cm_list"
      die "owned path not under named modules: $_cm_p"
    fi
  done < "$_cm_list"
  rm -f "$_cm_list"
  say MODULE-FIT
}

cmd_check_map_word() {
  _mw_file=${1:-}
  [ -n "$_mw_file" ] || die "usage: wm check-map-word RETURNFILE"
  [ -f "$_mw_file" ] || die "return file missing: $_mw_file"
  _mw_word=$(kv_get "$_mw_file" WORD)
  [ -n "$_mw_word" ] || die "worker returned no WORD"
  case $_mw_word in
    MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK) ;;
    *) die "map words are MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK (not CLOSED PASS)" ;;
  esac
  _mw_who=$(kv_get "$_mw_file" AGENT)
  if [ -z "$_mw_who" ]; then
    _mw_who=$(basename "$_mw_file" .md)
  fi
  case $_mw_who in
    parent|coordinator|loop|-|'') die "critique author id required" ;;
  esac
  ensure_wm
  _mw_mapper=$(mapper_id)
  [ -n "$_mw_mapper" ] || die "mapper id not recorded"
  _mw_map=$(kv_get "$_mw_file" MAP)
  [ -n "$_mw_map" ] || _mw_map=MAP.md
  if [ -f "$_mw_map" ]; then
    _mw_map_author=$(kv_get "$_mw_map" MAPPER)
    if [ -n "$_mw_map_author" ] && [ "$_mw_who" = "$_mw_map_author" ]; then
      die "critique must not write MAP-ACCEPT on a map it authored ($_mw_who)"
    fi
  fi
  if [ "$_mw_who" = "$_mw_mapper" ]; then
    die "architecture author id must differ from critique author id (both $_mw_who)"
  fi
  say "MAP-WORD $_mw_word author=$_mw_who"
}

cmd_map_ready() {
  [ -f MAP.md ] || die "MAP.md missing"
  _mr_mapper=$(mapper_id)
  [ -n "$_mr_mapper" ] || die "mapper id not recorded"
  cmd_check_module_fit
  materialize_slices PENDING
  say MAP-READY
}

cmd_map_verdict() {
  _mv_file=${1:-}
  [ -n "$_mv_file" ] || die "usage: wm map-verdict RETURNFILE"
  cmd_check_map_word "$_mv_file"
  _mv_word=$(kv_get "$_mv_file" WORD)
  _mv_who=$(kv_get "$_mv_file" AGENT)
  if [ -z "$_mv_who" ]; then
    _mv_who=$(basename "$_mv_file" .md)
  fi
  _mv_map=$(kv_get "$_mv_file" MAP)
  [ -n "$_mv_map" ] || _mv_map=MAP.md
  _mv_status=PENDING
  case $_mv_word in
    MAP-ACCEPT)
      cmd_check_module_fit
      _mv_status=READY
      ;;
    MAP-REVISE) _mv_status=REVISE ;;
    MAP-STOP-ASK) _mv_status=STOP-ASK ;;
  esac
  materialize_slices "$_mv_status"
  ensure_wm
  {
    printf 'WORD: %s\n' "$_mv_word"
    printf 'AGENT: %s\n' "$_mv_who"
    printf 'MAP: %s\n' "$_mv_map"
    printf 'when: %s\n' "$(iso_now)"
  } > "$WM/map-verdict"
  say "$_mv_word"
}

cmd_ready() {
  [ -f IDEA.md ] || die "IDEA.md missing"
  spec_ok || die "$SPEC_ERR"
  check_routing_batteries
  say READY
}

cmd_record_pre_falsify() {
  ensure_wm
  [ -f SPEC.md ] || die "SPEC.md missing"
  spec_ok || die "$SPEC_ERR"
  git rev-parse --verify HEAD >/dev/null 2>&1 || die "NOCOMMIT"
  spec_committed || die "SPEC.md must be committed before pre-falsify-wid"
  git rev-parse HEAD > "$WM/pre-falsify-wid"
  say "pre-falsify-wid $(cat "$WM/pre-falsify-wid")"
}

cmd_red() {
  ensure_wm
  [ -f "$WM/pre-falsify-wid" ] || die "pre-falsify-wid missing"
  spec_ok || die "$SPEC_ERR"
  _rd_fals=$(read_falsifier_cmd)
  check_falsifier_meta
  guard_command "$_rd_fals" falsifier
  if owned_porcelain_dirty; then
    die "dirty product porcelain"
  fi
  _rd_pre=$(cat "$WM/pre-falsify-wid")
  if product_commit_changed "$_rd_pre"; then
    printf 'early-implement\n' > "$WM/red.status"
    die "early implement"
  fi
  file_sha256 "$WM/FALSIFIER" > "$WM/FALSIFIER.sha256"
  set +e
  sh -c "$_rd_fals" > "$WM/red.out" 2>&1
  _rd_st=$?
  set -e
  if [ "$_rd_st" -eq 0 ]; then
    printf 'no-build\n' > "$WM/red.status"
    say NO-BUILD
    return 0
  fi
  printf 'red\n' > "$WM/red.status"
  git rev-parse HEAD > "$WM/pre-build-wid"
  say RED
}

cmd_built() {
  ensure_wm
  [ -f "$WM/pre-build-wid" ] || die "run red before build"
  _bu_wid=$(workid_short)
  [ "$_bu_wid" != NOCOMMIT ] || die "maker result requires a commit (work id is NOCOMMIT)"
  _bu_pre=$(cat "$WM/pre-build-wid")
  if head_eq_ref "$_bu_pre"; then
    printf 'fail\n' > "$WM/built.status"
    printf 'no-commit\n' > "$WM/built.reason"
    die "work id did not change after build — maker must commit"
  fi
  printf 'ok\n' > "$WM/built.status"
  rm -f "$WM/built.reason"
  write_last_maker_run "$(date +%s).$$" maker-build "$(date +%s)"
  say "BUILT $_bu_wid"
}

cmd_green() {
  ensure_wm
  [ -f "$WM/FALSIFIER" ] || die "FALSIFIER missing"
  if owned_porcelain_dirty; then
    die "dirty product porcelain"
  fi
  [ -f "$WM/FALSIFIER.sha256" ] || die "falsifier hash from red missing"
  _gr_now=$(file_sha256 "$WM/FALSIFIER")
  _gr_old=$(cat "$WM/FALSIFIER.sha256")
  [ "$_gr_now" = "$_gr_old" ] || die "falsifier mutated after observed-red"
  _gr_fals=$(read_falsifier_cmd)
  guard_command "$_gr_fals" falsifier
  set +e
  sh -c "$_gr_fals" > "$WM/green.out" 2>&1
  _gr_st=$?
  set -e
  if [ "$_gr_st" -ne 0 ]; then
    printf 'fail\n' > "$WM/green.status"
    die "falsifier still failing after build"
  fi
  printf 'ok\n' > "$WM/green.status"
  say GREEN
}

cmd_evidence() {
  _ev_who=${1:-}
  [ -n "$_ev_who" ] || die "usage: wm evidence AGENT -- CMD"
  shift
  [ "${1:-}" = -- ] && shift
  [ $# -ge 1 ] || die "usage: wm evidence AGENT -- CMD"
  ensure_wm
  _ev_join=
  for _ev_a in "$@"; do
    _ev_join="$_ev_join $_ev_a"
  done
  guard_command "$_ev_join" evidence
  _ev_wid=$(workid_short)
  _ev_stamp=$(date +%s)
  _ev_out="$WM/evidence/${_ev_who}.${_ev_stamp}.${_ev_wid}.txt"
  _ev_n=1
  while [ -e "$_ev_out" ]; do
    _ev_n=$((_ev_n + 1))
    _ev_out="$WM/evidence/${_ev_who}.${_ev_stamp}.${_ev_n}.${_ev_wid}.txt"
  done
  {
    printf 'working-mode-run/1\n'
    printf 'agent: %s\n' "$_ev_who"
    printf 'work-id: %s\n' "$_ev_wid"
    printf 'when: %s\n' "$(iso_now)"
    if [ -f "$WM/dispatch" ]; then
      printf 'dispatch-role: %s\n' "$(kv_get "$WM/dispatch" role)"
      printf 'dispatch-agent: %s\n' "$(kv_get "$WM/dispatch" agent)"
    else
      printf 'dispatch-role: none\n'
      printf 'dispatch-agent: -\n'
    fi
    printf 'command:'
    for _ev_a in "$@"; do
      printf ' %s' "$_ev_a"
    done
    printf '\n--- output ---\n'
    set +e
    "$@"
    _ev_st=$?
    set -e
    printf '\n--- status ---\nexit: %s\n' "$_ev_st"
  } > "$_ev_out" 2>&1
  say "$_ev_out"
}

cmd_verdict() {
  _vd_a=${1:-}
  _vd_b=${2:-}
  case $_vd_b in
    PASS|FAIL|BLOCKED|NO-BUILD)
      die "controller cannot stamp verdict word"
      ;;
  esac
  [ -n "$_vd_a" ] || die "usage: wm verdict RETURNFILE"
  [ -f "$_vd_a" ] || die "return file missing: $_vd_a"
  _vd_word=$(kv_get "$_vd_a" WORD)
  _vd_ev=$(kv_get "$_vd_a" EVIDENCE)
  _vd_reason=$(kv_get "$_vd_a" REASON)
  [ -n "$_vd_word" ] || die "worker returned no WORD"
  case $_vd_word in
    PASS|FAIL|BLOCKED|NO-BUILD) ;;
    *) die "invalid WORD: $_vd_word" ;;
  esac
  _vd_who=$(basename "$_vd_a" .md)
  _vd_maker=$(panel_agent maker)
  _vd_rev=$(panel_agent reviewer)
  _vd_scout=$(panel_agent scout)
  case $_vd_who in
    parent|coordinator|loop) die "parent/coordinator/loop cannot author a verdict" ;;
  esac
  [ "$_vd_who" != "$_vd_maker" ] || die "no recorded maker may judge the item they helped build ($_vd_who)"
  _vd_ok_author=0
  [ "$_vd_who" = "$_vd_rev" ] && _vd_ok_author=1
  [ "$_vd_who" = "$_vd_scout" ] && _vd_ok_author=1
  [ "$_vd_ok_author" -eq 1 ] || die "unknown/uncast agent is not a verdict author"
  if [ "$_vd_who" = "$_vd_scout" ] && [ "$_vd_word" != NO-BUILD ]; then
    die "scout may only author NO-BUILD"
  fi
  [ -n "$_vd_ev" ] || die "evidence missing"
  [ -f "$_vd_ev" ] || die "evidence file missing: $_vd_ev"
  grep -q '^working-mode-run/1$' "$_vd_ev" || die "evidence missing tool header (must be recorded by wm evidence)"
  grep -q "^agent: ${_vd_who}$" "$_vd_ev" || die "a PASS/verdict must name evidence its author recorded (agent $_vd_who)"
  _vd_wid=$(workid_short)
  [ "$_vd_wid" != NOCOMMIT ] || {
    [ "$_vd_word" != PASS ] || die "PASS refuses NOCOMMIT work id"
  }
  grep -q "^work-id: " "$_vd_ev" || die "evidence missing work-id"
  if [ "$_vd_word" = PASS ]; then
    [ -f "$WM/green.status" ] && [ "$(cat "$WM/green.status")" = ok ] || die "PASS requires green.status=ok"
    _vd_fals=$(read_falsifier_cmd)
    grep -F "$_vd_fals" "$_vd_ev" >/dev/null || die "PASS requires the reviewer to have run the named falsifier"
    grep -q '^exit: 0$' "$_vd_ev" || die "PASS requires falsifier exit 0"
  fi
  if [ "$_vd_word" = NO-BUILD ]; then
    if owned_porcelain_dirty; then
      die "dirty product porcelain"
    fi
    if [ -f "$WM/pre-falsify-wid" ] && product_commit_changed "$(cat "$WM/pre-falsify-wid")"; then
      die "NO-BUILD with product-path changes"
    fi
  fi
  _vd_iso=$(honest_isolation)
  ensure_wm
  mkdir -p "$WM/verdicts"
  {
    printf 'VERDICT: %s\n' "$_vd_word"
    printf 'AGENT: %s\n' "$_vd_who"
    printf 'WORK-ID: %s\n' "$_vd_wid"
    printf 'EVIDENCE: %s\n' "$_vd_ev"
    printf 'INGEST: return\n'
    _vd_st="$WM/spawn/${_vd_who}.stamp"
    if [ -f "$_vd_st" ]; then
      _vd_rid=$(kv_get "$_vd_st" run-id)
      if [ -n "$_vd_rid" ]; then
        printf 'RUN-ID: %s\n' "$_vd_rid"
      fi
    fi
    printf 'ISOLATION: %s\n' "$_vd_iso"
    printf 'MODEL-SWITCH: UNVERIFIED\n'
    if [ -n "$_vd_reason" ]; then
      printf 'REASON: %s\n' "$_vd_reason"
    fi
  } > "$WM/verdicts/${_vd_who}.md"
  say "$WM/verdicts/${_vd_who}.md"
}

cmd_close() {
  _cl_lesson=${1:-}
  [ -n "$_cl_lesson" ] || die "close without a lesson line"
  _cl_nl=$(printf '%s' "$_cl_lesson" | wc -l | tr -d ' ')
  [ "$_cl_nl" = 0 ] || die "lesson must be one line"
  ensure_wm
  if closed_is_closeable; then
    die "already closed"
  fi
  [ -d "$WM/verdicts" ] || die "no verdicts"
  _cl_pass=0
  _cl_nobuild=0
  for _cv in "$WM/verdicts"/*.md; do
    [ -f "$_cv" ] || continue
    _cv_word=$(kv_get "$_cv" VERDICT)
    _cv_who=$(kv_get "$_cv" AGENT)
    _cl_maker=$(panel_agent maker)
    case $_cv_who in
      parent|coordinator|loop) die "stored verdict authored by parent/coordinator/loop" ;;
    esac
    [ "$_cv_who" != "$_cl_maker" ] || die "stored verdict authored by maker $_cv_who"
    if [ "$_cv_word" = PASS ]; then
      verdict_is_closeable "$_cv" || die "PASS requires a reviewer-role run (invoke.log after last maker-*); planted verdict is not PASS"
      _cl_pass=1
    fi
    if [ "$_cv_word" = NO-BUILD ]; then
      if owned_porcelain_dirty; then
        die "dirty product porcelain"
      fi
      if [ -f "$WM/pre-falsify-wid" ] && product_commit_changed "$(cat "$WM/pre-falsify-wid")"; then
        die "NO-BUILD with product-path changes"
      fi
      verdict_is_closeable "$_cv" || die "NO-BUILD requires a reviewer-role run; planted verdict is not NO-BUILD"
      _cl_nobuild=1
    fi
  done
  if [ "$_cl_pass" -eq 1 ]; then
    [ -f "$WM/green.status" ] && [ "$(cat "$WM/green.status")" = ok ] || die "PASS path requires green.status=ok"
    printf 'CLOSED PASS\n' > "$WM/CLOSED"
    say "CLOSED PASS"
    if close_append_lesson "$_cl_lesson"; then
      return 0
    fi
    return 1
  fi
  if [ "$_cl_nobuild" -eq 1 ]; then
    printf 'CLOSED NO-BUILD\n' > "$WM/CLOSED"
    say "CLOSED NO-BUILD"
    if close_append_lesson "$_cl_lesson"; then
      return 0
    fi
    return 1
  fi
  die "close without PASS or worker NO-BUILD"
}

# Exactly one line per successful close. ARCH: → STOP-ASK (nonzero).
close_append_lesson() {
  _ca_lesson=$1
  _ca_lf=$(lessons_file)
  case $_ca_lf in
    .crucible/*)
      _ca_dir=$(dirname "$_ca_lf")
      [ -d "$_ca_dir" ] || die "program dir missing for LESSONS"
      ;;
  esac
  printf '%s\n' "$_ca_lesson" >> "$_ca_lf"
  case $_ca_lesson in
    ARCH:*)
      say "STOP-ASK ARCH"
      return 1
      ;;
  esac
  return 0
}

cmd_next() {
  if [ ! -f IDEA.md ] || [ ! -s IDEA.md ]; then
    say "NEXT INTAKE"
    return 0
  fi
  if ! panel_valid; then
    say "NEXT CAST"
    return 0
  fi
  if [ -f MAP.md ] || [ -f slices.tsv ]; then
    _nx_mw=$(map_word_recorded)
    if [ -z "$_nx_mw" ]; then
      say "NEXT MAP"
      return 0
    fi
    if [ "$_nx_mw" = MAP-REVISE ]; then
      say "NEXT MAP"
      return 0
    fi
    if [ "$_nx_mw" = MAP-STOP-ASK ]; then
      say "STOP-ASK"
      return 0
    fi
    if [ "$_nx_mw" = MAP-ACCEPT ]; then
      if [ ! -f "$WM/FALSIFIER" ] && [ ! -f "$WM/pre-falsify-wid" ]; then
        if map_needs_human_sign && ! human_sign_present; then
          say "STOP-ASK MAP-HUMAN"
          return 0
        fi
        _nx_sid=$(first_ready_slice)
        _nx_risk=
        if [ -n "$_nx_sid" ]; then
          _nx_risk=$(slice_risk "$_nx_sid")
        fi
        if [ "$_nx_risk" = HIGH ] && ! high_kinds_ok; then
          say "STOP-ASK"
          return 0
        fi
        if [ -n "$_nx_sid" ]; then
          if arch_fence_active; then
            say "STOP-ASK ARCH"
            return 0
          fi
          say "NEXT SLICE $_nx_sid"
          return 0
        fi
      fi
    fi
  fi
  if [ ! -f SPEC.md ] || ! spec_ok; then
    if research_skill_present && role_has_cli specifier && [ ! -f RESEARCH.md ]; then
      say "NEXT RESEARCH"
      return 0
    fi
    say "NEXT SPEC"
    return 0
  fi
  if arch_fence_active; then
    say "STOP-ASK ARCH"
    return 0
  fi
  if closed_is_closeable; then
    _nx_more=$(first_ready_slice)
    if [ -z "$_nx_more" ]; then
      say DONE
      return 0
    fi
  fi
  if spec_committed && [ ! -f "$WM/pre-falsify-wid" ]; then
    say "NEXT RECORD PRE-FALSIFY"
    return 0
  fi
  if live_is_pass_or_nobuild; then
    say "NEXT CLOSE"
    return 0
  fi
  if [ ! -f "$WM/FALSIFIER" ]; then
    say "NEXT RUN maker-falsify"
    return 0
  fi
  if [ ! -f "$WM/red.status" ]; then
    say "NEXT RED"
    return 0
  fi
  _nx_red=$(cat "$WM/red.status")
  if [ "$_nx_red" = early-implement ]; then
    say "ESCALATE EARLY_IMPLEMENT"
    return 0
  fi
  if [ "$_nx_red" = no-build ]; then
    say "NEXT RUN reviewer"
    return 0
  fi
  _nx_bs=
  [ -f "$WM/built.status" ] && _nx_bs=$(cat "$WM/built.status")
  _nx_preb=
  [ -f "$WM/pre-build-wid" ] && _nx_preb=$(cat "$WM/pre-build-wid")
  _nx_gs=
  [ -f "$WM/green.status" ] && _nx_gs=$(cat "$WM/green.status")
  if [ "$_nx_gs" = fail ]; then
    say "NEXT RUN maker-build"
    return 0
  fi
  if [ "$_nx_red" = red ] && [ -z "$_nx_bs" ] && head_eq_ref "$_nx_preb"; then
    say "NEXT RUN maker-build"
    return 0
  fi
  if [ "$_nx_red" = red ] && [ -z "$_nx_bs" ] && ! head_eq_ref "$_nx_preb"; then
    say "NEXT BUILT"
    return 0
  fi
  if [ "$_nx_bs" = ok ] && [ -z "$_nx_gs" ]; then
    say "NEXT GREEN"
    return 0
  fi
  if [ "$_nx_gs" = ok ]; then
    say "NEXT RUN reviewer"
    return 0
  fi
  say "NEXT RUN reviewer"
}

cmd_status() { cmd_next; }

cmd_run() {
  _ru_role=${1:-}
  [ -n "$_ru_role" ] || die "usage: wm run ROLE"
  ensure_wm
  _ru_prole=$_ru_role
  case $_ru_role in
    maker-falsify|maker-build) _ru_prole=maker ;;
  esac
  _ru_agent=$(panel_agent "$_ru_prole")
  [ -n "$_ru_agent" ] && [ "$_ru_agent" != - ] || die "role $_ru_prole is not cast"
  case $_ru_agent in
    parent|coordinator|loop)
      die "maker or reviewer cannot be parent/coordinator/loop"
      ;;
  esac
  if [ "$_ru_prole" = maker ] || [ "$_ru_prole" = reviewer ]; then
    _ru_other=
    if [ "$_ru_prole" = maker ]; then
      _ru_other=$(panel_agent reviewer)
    else
      _ru_other=$(panel_agent maker)
    fi
    [ "$_ru_agent" != "$_ru_other" ] || die "maker and reviewer must be distinct"
  fi
  if [ "$_ru_prole" = maker ]; then
    refuse_if_mapper_is_maker "$_ru_agent"
    refuse_if_specifier_is_maker "$_ru_agent"
    guard_map_before_maker
    if arch_fence_active; then
      die "STOP-ASK ARCH"
    fi
  fi
  if [ "$_ru_prole" = specifier ]; then
    case $_ru_agent in
      parent|coordinator|loop) die "specifier cannot be parent/coordinator/loop" ;;
    esac
    _ru_maker=$(panel_agent maker)
    if [ -n "$_ru_maker" ] && [ "$_ru_maker" != - ] && [ "$_ru_agent" = "$_ru_maker" ]; then
      die "specifier cannot be maker ($_ru_agent)"
    fi
  fi
  _ru_command=$(panel_cmd "$_ru_prole")
  if [ -z "$_ru_command" ] || [ "$_ru_command" = - ]; then
    die "INDEPENDENCE_UNAVAILABLE: no CLI worker"
  fi
  case $_ru_command in
    *host-spawn*)
      die "host-spawn waiter"
      ;;
  esac
  guard_command "$_ru_command" "$_ru_prole"
  if [ "$_ru_role" = maker-falsify ] && [ -f "$WM/FALSIFIER" ]; then
    die "FALSIFIER exists before maker-falsify (controller authorship)"
  fi
  _ru_judge_snap=
  if [ "$_ru_role" = maker-falsify ] || [ "$_ru_role" = maker-build ]; then
    _ru_judge_snap="$WM/.judge.snap.$$"
    snapshot_judge_artifacts "$_ru_judge_snap"
  fi
  _ru_brief=$(write_brief "$_ru_role" "$_ru_agent")
  [ -f "$_ru_brief" ] || die "brief missing"
  printf 'role: %s\nagent: %s\n' "$_ru_role" "$_ru_agent" > "$WM/dispatch"
  if [ "$_ru_role" = reviewer ] || [ "$_ru_role" = scout ]; then
    _ru_ret="$WM/return/${_ru_agent}.md"
    if [ -f "$_ru_ret" ]; then
      mkdir -p "$WM/return/history"
      mv "$_ru_ret" "$WM/return/history/${_ru_agent}.$(date +%s).md"
    fi
    mkdir -p "$WM/spawn"
    _ru_t=$(date +%s)
    printf 't: %s\nabsent_return: yes\npath: %s\nrole: %s\nrun-id: %s.%s\n' \
      "$_ru_t" "$_ru_ret" "$_ru_role" "$_ru_t" "$$" > "$WM/spawn/${_ru_agent}.stamp"
  fi
  _ru_absbrief=$PWD/$_ru_brief
  case $_ru_brief in
    /*) _ru_absbrief=$_ru_brief ;;
  esac
  # Quote inside awk from ENVIRON. awk -v unescapes \" so a " in the
  # path splits sh -c. Escape \, ", $, and ` for POSIX double quotes.
  WM_BRIEF=$_ru_absbrief
  export WM_BRIEF
  _ru_expanded=$(printf '%s\n' "$_ru_command" | awk '
    function quote_dq(s,    i, n, c, out) {
      n = length(s)
      out = "\""
      for (i = 1; i <= n; i++) {
        c = substr(s, i, 1)
        if (c == "\\" || c == "\"" || c == "$" || c == "`")
          out = out "\\"
        out = out c
      }
      return out "\""
    }
    BEGIN { b = quote_dq(ENVIRON["WM_BRIEF"]) }
    {
      s = $0
      out = ""
      while ((i = index(s, "{BRIEF}")) > 0) {
        out = out substr(s, 1, i - 1) b
        s = substr(s, i + 7)
      }
      print out s
    }
  ')
  unset WM_BRIEF
  BRIEF=$_ru_absbrief
  export BRIEF
  if [ "$_ru_role" = reviewer ] || [ "$_ru_role" = scout ]; then
    write_invoke_log "$_ru_role" "$_ru_agent" "$_ru_expanded" "$$"
  fi
  set +e
  sh -c "$_ru_expanded" > "$WM/worker.out" 2>"$WM/worker.err"
  _ru_rc=$?
  set -e
  rm -f "$WM/dispatch"
  if [ "$_ru_role" = maker-falsify ] || [ "$_ru_role" = maker-build ]; then
    write_last_maker_run "$(date +%s).$$" "$_ru_role" "$(date +%s)"
    if [ -n "$_ru_judge_snap" ] && judge_artifacts_changed "$_ru_judge_snap"; then
      rm -f "$_ru_judge_snap"
      die "maker wrote judge artifacts"
    fi
    rm -f "$_ru_judge_snap"
  fi
  [ -f "$_ru_brief" ] || die "brief missing after wait"
  if [ "$_ru_role" = maker-falsify ]; then
    [ -f "$WM/FALSIFIER" ] || die "maker-falsify did not write FALSIFIER"
    check_falsifier_meta
  fi
  if [ "$_ru_role" = reviewer ] || [ "$_ru_role" = scout ]; then
    _ru_ret="$WM/return/${_ru_agent}.md"
    [ -f "$_ru_ret" ] || die "worker returned no WORD"
    _ru_st="$WM/spawn/${_ru_agent}.stamp"
    [ -f "$_ru_st" ] || die "spawn stamp missing"
    grep -q 'absent_return: yes' "$_ru_st" || die "planted WORD file"
    _ru_t=$(kv_get "$_ru_st" t)
    _ru_mt=$(file_mtime "$_ru_ret")
    if [ -n "$_ru_t" ] && [ "$_ru_mt" -lt "$_ru_t" ]; then
      die "planted WORD file"
    fi
    grep -q '^WORD:' "$_ru_ret" || die "worker returned no WORD"
    _ru_word=$(kv_get "$_ru_ret" WORD)
    if [ "$_ru_role" = reviewer ] && [ -f "$WM/red.status" ] \
      && [ "$(cat "$WM/red.status")" = no-build ]; then
      [ "$_ru_word" = NO-BUILD ] || die "NO-BUILD red cannot PASS"
    fi
    if [ "$_ru_role" = scout ]; then
      case $_ru_word in
        MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK)
          cmd_map_verdict "$_ru_ret"
          return "$_ru_rc"
          ;;
      esac
    fi
    cmd_verdict "$_ru_ret"
  fi
  return "$_ru_rc"
}

cmd_loop() {
  # Foreground walker: consume next until work-level CLOSE / STOP-ASK / ESCALATE.
  # One slice in flight. After a closeable brick, mark that slice CLOSED and
  # continue remaining READY slices (deps satisfied). Exec PANEL as children.
  [ -x "$PWD/$WM/bin/wm" ] || die "missing .wm/bin/wm (run wm init)"
  WM_BIN="$PWD/$WM/bin/wm"
  PATH="$PWD/$WM/bin:$PATH"
  export PATH
  _lp_i=0
  _lp_ran_reviewer=0
  _lp_slice=
  _lp_lesson=$(resolve_loop_lesson)
  if [ -f "$WM/slice-in-flight" ]; then
    _lp_slice=$(kv_get "$WM/slice-in-flight" id)
  fi
  while :; do
    _lp_i=$((_lp_i + 1))
    # LOOP_BOUND 40: specifier + scout + one LOW brick is ~12 cards;
    # three CLOSED slices stay under 40. Raise to 80 if a longer map needs it.
    if [ "$_lp_i" -gt 40 ]; then
      say "ESCALATE LOOP_BOUND"
      exit 1
    fi
    if closed_is_closeable; then
      _lp_more=$(first_ready_slice)
      if [ -z "$_lp_more" ]; then
        cat "$WM/CLOSED"
        exit 0
      fi
    fi
    _lp_card=$("$WM_BIN" next) || exit 1
    _lp_card=$(printf '%s\n' "$_lp_card" | awk 'NF { print; exit }')
    case $_lp_card in
      "NEXT INTAKE"|"NEXT CAST")
        say "STOP-ASK $_lp_card"
        exit 1
        ;;
      "NEXT RESEARCH")
        if ! role_has_cli specifier; then
          say "STOP-ASK NEXT RESEARCH"
          exit 1
        fi
        say "$_lp_card"
        "$WM_BIN" run specifier || exit 1
        if [ -s QUESTIONS.md ]; then
          say "STOP-ASK QUESTIONS"
          exit 1
        fi
        if [ ! -s RESEARCH.md ]; then
          say "STOP-ASK RESEARCH incomplete"
          exit 1
        fi
        ;;
      "NEXT SPEC")
        if ! role_has_cli specifier; then
          say "STOP-ASK NEXT SPEC"
          exit 1
        fi
        say "$_lp_card"
        "$WM_BIN" run specifier || exit 1
        if [ -s QUESTIONS.md ]; then
          say "STOP-ASK QUESTIONS"
          exit 1
        fi
        if ! spec_ok; then
          say "STOP-ASK SPEC incomplete"
          exit 1
        fi
        ;;
      "NEXT MAP")
        if [ ! -f MAP.md ]; then
          if ! role_has_cli specifier; then
            say "STOP-ASK NEXT MAP"
            exit 1
          fi
          say "$_lp_card"
          "$WM_BIN" run specifier || exit 1
          if [ -s QUESTIONS.md ]; then
            say "STOP-ASK QUESTIONS"
            exit 1
          fi
          if [ ! -f MAP.md ]; then
            say "STOP-ASK NEXT MAP"
            exit 1
          fi
        else
          say "$_lp_card"
        fi
        "$WM_BIN" record-mapper --from MAP.md || exit 1
        "$WM_BIN" map-ready || exit 1
        _lp_mw=$(map_word_recorded)
        if [ -z "$_lp_mw" ] || [ "$_lp_mw" = MAP-REVISE ]; then
          if ! role_has_cli scout; then
            say "STOP-ASK NEXT MAP"
            exit 1
          fi
          "$WM_BIN" run scout || exit 1
          _lp_scout=$(panel_agent scout)
          "$WM_BIN" map-verdict "$WM/return/${_lp_scout}.md" || exit 1
        fi
        ;;
      "NEXT SLICE "*)
        _lp_sid=${_lp_card#NEXT SLICE }
        [ -n "$_lp_sid" ] || { say "STOP-ASK NEXT SLICE"; exit 1; }
        if [ -n "$_lp_slice" ] && [ "$_lp_slice" != "$_lp_sid" ]; then
          say "STOP-ASK one slice in flight ($_lp_slice)"
          exit 1
        fi
        say "$_lp_card"
        _lp_slice=$_lp_sid
        ensure_wm
        printf 'id: %s\n' "$_lp_sid" > "$WM/slice-in-flight"
        if [ ! -f SPEC.md ]; then
          say "STOP-ASK SPEC incomplete"
          exit 1
        fi
        "$WM_BIN" record-pre-falsify || exit 1
        ;;
      STOP-ASK|STOP-ASK*)
        say "$_lp_card"
        exit 1
        ;;
      ESCALATE*)
        say "$_lp_card"
        exit 1
        ;;
      "NEXT RECORD PRE-FALSIFY")
        "$WM_BIN" record-pre-falsify || exit 1
        ;;
      "NEXT RUN maker-falsify")
        "$WM_BIN" run maker-falsify || exit 1
        ;;
      "NEXT RED")
        set +e
        "$WM_BIN" red
        _lp_rc=$?
        set -e
        if [ "$_lp_rc" -ne 0 ]; then
          _lp_n2=$("$WM_BIN" next) || true
          _lp_n2=$(printf '%s\n' "$_lp_n2" | awk 'NF { print; exit }')
          case $_lp_n2 in
            ESCALATE*) say "$_lp_n2"; exit 1 ;;
            STOP-ASK*) say "$_lp_n2"; exit 1 ;;
            *) say "STOP-ASK red refused"; exit 1 ;;
          esac
        fi
        ;;
      "NEXT RUN scout")
        "$WM_BIN" run scout || exit 1
        ;;
      "NEXT RUN maker-build")
        "$WM_BIN" run maker-build || exit 1
        ;;
      "NEXT BUILT")
        "$WM_BIN" built || exit 1
        ;;
      "NEXT GREEN")
        set +e
        "$WM_BIN" green
        set -e
        ;;
      "NEXT RUN reviewer")
        "$WM_BIN" run reviewer || exit 1
        _lp_ran_reviewer=1
        ;;
      "NEXT CLOSE")
        if [ "$_lp_ran_reviewer" -eq 0 ]; then
          "$WM_BIN" run reviewer || exit 1
          _lp_ran_reviewer=1
        fi
        "$WM_BIN" close "$_lp_lesson" || exit 1
        if arch_fence_active; then
          say "STOP-ASK ARCH"
          exit 1
        fi
        if [ -n "$_lp_slice" ]; then
          mark_slice_status "$_lp_slice" CLOSED
        fi
        _lp_next=$(first_ready_slice)
        if [ -n "$_lp_next" ]; then
          reset_brick
          _lp_ran_reviewer=0
          _lp_slice=
          continue
        fi
        cat "$WM/CLOSED"
        exit 0
        ;;
      DONE)
        if closed_is_closeable; then
          _lp_more=$(first_ready_slice)
          if [ -z "$_lp_more" ]; then
            cat "$WM/CLOSED"
            exit 0
          fi
        fi
        say "STOP-ASK DONE without closeable CLOSED"
        exit 1
        ;;
      INDEPENDENCE_UNAVAILABLE*)
        say "$_lp_card"
        exit 1
        ;;
      *)
        say "STOP-ASK unknown next card: $_lp_card"
        exit 1
        ;;
    esac
  done
}

WM_GO=$(CDPATH= cd "$(dirname "$0")" && pwd)/wm-go.sh
[ -f "$WM_GO" ] && . "$WM_GO"

cmd=${1:-}
if [ -z "$cmd" ]; then
  if type cmd_help >/dev/null 2>&1; then
    cmd_help
    exit 0
  fi
  die "usage: wm <command>"
fi
shift
case $cmd in
  go|bootstrap)
    type cmd_go >/dev/null 2>&1 || die "refresh from 1.8.0"
    cmd_go "$@"
    ;;
  help)
    type cmd_help >/dev/null 2>&1 || die "refresh from 1.8.0"
    cmd_help
    ;;
  init) cmd_init "$@" ;;
  cast) cmd_cast "$@" ;;
  record-mapper) cmd_record_mapper "$@" ;;
  check-module-fit) cmd_check_module_fit "$@" ;;
  check-map-word) cmd_check_map_word "$@" ;;
  map-ready) cmd_map_ready "$@" ;;
  map-verdict) cmd_map_verdict "$@" ;;
  ready) cmd_ready "$@" ;;
  workid) cmd_workid "$@" ;;
  record-pre-falsify) cmd_record_pre_falsify "$@" ;;
  red) cmd_red "$@" ;;
  built) cmd_built "$@" ;;
  green) cmd_green "$@" ;;
  evidence) cmd_evidence "$@" ;;
  verdict) cmd_verdict "$@" ;;
  close) cmd_close "$@" ;;
  next) cmd_next "$@" ;;
  status) cmd_status "$@" ;;
  run) cmd_run "$@" ;;
  loop) cmd_loop "$@" ;;
  *) die "unknown command: $cmd" ;;
esac
