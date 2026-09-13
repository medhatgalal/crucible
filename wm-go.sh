# wm-go.sh — sourced by wm.sh (same directory). Not a standalone entrypoint.
# go discovers grok/claude/codex, casts distinct ids, then cmd_loop.

_go_cli_cmd() {
  case $1 in
    grok) printf '%s\n' 'grok -p --prompt-file {BRIEF}' ;;
    claude) printf '%s\n' 'claude -p --output-format text read {BRIEF} and follow it exactly' ;;
    codex) printf '%s\n' 'codex exec -- read {BRIEF} and follow it exactly' ;;
    *) die "INDEPENDENCE_UNAVAILABLE: no CLI worker" ;;
  esac
}

cmd_help() {
  _ch_prog=work
  if [ -f .crucible/work/wm.sh ]; then
    _ch_prog=work
  else
    for _ch_cand in .crucible/*/wm.sh; do
      [ -f "$_ch_cand" ] || continue
      _ch_dir=${_ch_cand%/*}
      _ch_prog=${_ch_dir##*/}
      break
    done
  fi
  _ch_next=$(cmd_next)
  _ch_next=$(printf '%s\n' "$_ch_next" | awk 'NF { print; exit }')
  say "working-mode"
  say "run: .crucible/${_ch_prog}/wm.sh go [IDEA.md]"
  say "next: ${_ch_next}"
  say "commands: go help status init cast loop"
  say "harness: read WORKING-MODE.md then go"
}

cmd_go() {
  _go_idea=${1:-}
  if [ ! -x "$WM/bin/wm" ]; then
    cmd_init
  fi
  if [ -n "$_go_idea" ] && [ -f "$_go_idea" ] && [ -r "$_go_idea" ] && [ ! -f IDEA.md ]; then
    cp "$_go_idea" IDEA.md
  fi
  if [ -s QUESTIONS.md ]; then
    say "STOP-ASK QUESTIONS"
    exit 1
  fi
  if ! panel_valid; then
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
    [ "$_go_n" -gt 0 ] || die "INDEPENDENCE_UNAVAILABLE: no CLI worker"
    [ -n "$_go_k2" ] || _go_k2=$_go_k1
    _go_spec_k=$_go_k1
    _go_scout_k=$_go_k2
    _go_make_k=$_go_k1
    _go_rev_k=$_go_k2
    cmd_cast specifier spec0 "$_go_spec_k" "$(_go_cli_cmd "$_go_spec_k")"
    cmd_cast scout scout0 "$_go_scout_k" "$(_go_cli_cmd "$_go_scout_k")"
    cmd_cast maker make0 "$_go_make_k" "$(_go_cli_cmd "$_go_make_k")"
    cmd_cast reviewer rev0 "$_go_rev_k" "$(_go_cli_cmd "$_go_rev_k")"
  fi
  cmd_loop
}
