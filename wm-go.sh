# wm-go.sh — sourced by wm.sh (same directory). Not a standalone entrypoint.
# go discovers grok/claude/codex, casts distinct ids, then cmd_loop.

_go_cli_cmd() {
  case $1 in
    grok) printf '%s\n' 'grok -p --prompt-file {BRIEF}' ;;
    claude) printf '%s\n' "claude -p --output-format text 'read {BRIEF} and follow it exactly'" ;;
    codex) printf '%s\n' "codex exec -- 'read {BRIEF} and follow it exactly'" ;;
    *) die "INDEPENDENCE_UNAVAILABLE: no CLI worker" ;;
  esac
}

cmd_help() {
  _ch_prog=work
  _ch_from0=
  case $0 in
    */.crucible/*/wm.sh|.crucible/*/wm.sh)
      _ch_dir=${0%/*}
      _ch_cand=${_ch_dir##*/}
      case $_ch_cand in
        *[*]*|'') ;;
        *)
          _ch_prog=$_ch_cand
          _ch_from0=1
          ;;
      esac
      ;;
  esac
  if [ -z "$_ch_from0" ]; then
    if [ -f .crucible/work/wm.sh ]; then
      _ch_prog=work
    else
      for _ch_cand in .crucible/*/wm.sh; do
        case $_ch_cand in *[*]*) continue ;; esac
        [ -f "$_ch_cand" ] || continue
        _ch_dir=${_ch_cand%/*}
        _ch_prog=${_ch_dir##*/}
        case $_ch_prog in *[*]*) _ch_prog=work ;; esac
        break
      done
    fi
  fi
  _ch_next=$(cmd_next)
  _ch_next=$(printf '%s\n' "$_ch_next" | awk 'NF { print; exit }')
  say "working-mode"
  say "run: .crucible/${_ch_prog}/wm.sh go [IDEA.md]"
  say "next: ${_ch_next}"
  say "commands: go help status init cast loop bound"
  say "harness: read WORKING-MODE.md then go"
}

go_ensure_panel() {
  _need_mr=0
  _need_spec=0
  _need_scout=0
  panel_valid || _need_mr=1
  role_has_cli specifier || _need_spec=1
  role_has_cli scout || _need_scout=1
  if [ "$_need_mr" -eq 0 ] && [ "$_need_spec" -eq 0 ] && [ "$_need_scout" -eq 0 ]; then
    return 0
  fi
  _go_n=0
  _go_k1=
  _go_k2=
  for _go_cli in grok claude codex; do
    if command -v "$_go_cli" >/dev/null 2>&1; then
      _go_n=$((_go_n + 1))
      if [ -z "$_go_k1" ]; then
        _go_k1=$_go_cli
      elif [ -z "$_go_k2" ]; then
        _go_k2=$_go_cli
      fi
    fi
  done
  if [ "$_go_n" -eq 0 ]; then
    [ "$_need_mr" -eq 0 ] || die "INDEPENDENCE_UNAVAILABLE: no CLI worker"
    return 0
  fi
  [ -n "$_go_k2" ] || _go_k2=$_go_k1
  if [ "$_need_mr" -eq 1 ]; then
    cmd_cast specifier spec0 "$_go_k1" "$(_go_cli_cmd "$_go_k1")"
    cmd_cast scout scout0 "$_go_k2" "$(_go_cli_cmd "$_go_k2")"
    cmd_cast maker make0 "$_go_k1" "$(_go_cli_cmd "$_go_k1")"
    cmd_cast reviewer rev0 "$_go_k2" "$(_go_cli_cmd "$_go_k2")"
    return 0
  fi
  if [ "$_need_spec" -eq 1 ]; then
    cmd_cast specifier spec0 "$_go_k1" "$(_go_cli_cmd "$_go_k1")"
  fi
  if [ "$_need_scout" -eq 1 ]; then
    cmd_cast scout scout0 "$_go_k2" "$(_go_cli_cmd "$_go_k2")"
  fi
}

cmd_go() {
  _go_next=0
  _go_idea=
  while [ $# -gt 0 ]; do
    case $1 in
      --next) _go_next=1 ;;
      -*) die "idea path must not start with -" ;;
      *)
        if [ -n "$_go_idea" ]; then
          die "usage: go [--next] [IDEA.md]"
        fi
        _go_idea=$1
        ;;
    esac
    shift
  done
  if [ "$_go_next" -eq 1 ] && [ -n "$_go_idea" ]; then
    die "go --next does not take an idea path"
  fi
  if [ ! -x "$WM/bin/wm" ]; then
    cmd_init
  fi
  if [ "$_go_next" -eq 1 ]; then
    go_consume_backlog
  elif [ -n "$_go_idea" ]; then
    if [ -f "$_go_idea" ] && [ -r "$_go_idea" ] && [ ! -f IDEA.md ]; then
      cp "$_go_idea" ./IDEA.md
    fi
  fi
  if questions_need_ask; then
    metrics_append "STOP-ASK QUESTIONS" -
    say "STOP-ASK QUESTIONS"
    exit 1
  fi
  go_ensure_panel
  cmd_loop
}

WM_GO_LOADED=1
