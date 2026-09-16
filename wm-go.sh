# wm-go.sh — sourced by wm.sh (same directory). Not a standalone entrypoint.
# go discovers grok/kiro-cli/codex, casts distinct ids, then cmd_loop.
# kiro-cli binary → kind kiro. Do not use `kiro-cli acp` as a wm run argv.
# Live kiro auth uses host HOME (keychain); empty HOME hang is not logout.

_go_cli_cmd() {
  case $1 in
    grok) printf '%s\n' 'grok --session-id {SESSION} --no-subagents -p --prompt-file {BRIEF}' ;;
    kiro) printf '%s\n' "kiro-cli chat --no-interactive --trust-all-tools 'read {BRIEF} and follow it exactly'" ;;
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
  say "commands: go status help"
  say "debug: init cast loop bound next"
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
  # Roster: grok, kiro-cli (kind kiro), codex. Two kinds = any two of those.
  for _go_cli in grok kiro-cli codex; do
    if command -v "$_go_cli" >/dev/null 2>&1; then
      case $_go_cli in
        kiro-cli) _go_kind=kiro ;;
        *) _go_kind=$_go_cli ;;
      esac
      _go_n=$((_go_n + 1))
      if [ -z "$_go_k1" ]; then
        _go_k1=$_go_kind
      elif [ -z "$_go_k2" ]; then
        _go_k2=$_go_kind
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
  else
    if [ ! -f IDEA.md ] || [ ! -s IDEA.md ]; then
      if backlog_has_ready; then
        go_consume_backlog
      else
        metrics_append "STOP-ASK INTAKE" -
        floor_write "STOP-ASK INTAKE"
        say "STOP-ASK INTAKE"
        exit 1
      fi
    elif closed_is_closeable && backlog_has_ready; then
      go_consume_backlog
    fi
  fi
  if questions_need_ask; then
    metrics_append "STOP-ASK QUESTIONS" -
    floor_write "STOP-ASK QUESTIONS"
    say "STOP-ASK QUESTIONS"
    exit 1
  fi
  go_ensure_panel
  _go_preview=$(cmd_next)
  _go_preview=$(printf '%s\n' "$_go_preview" | awk 'NF { print; exit }')
  floor_write "$_go_preview"
  cmd_loop
}

WM_GO_LOADED=1
