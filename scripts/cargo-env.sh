#!/bin/sh
# Keep rustup/cargo dirs when HOME is redirected for empty-HOME CHECKs.
# Source this file; do not exec it.
if [ -z "${CARGO_HOME:-}" ]; then
  _cb=$(command -v cargo) || true
  case $_cb in
    */bin/cargo)
      CARGO_HOME=$(CDPATH=; cd "$(dirname "$_cb")/.." && pwd)
      export CARGO_HOME
      ;;
  esac
fi
if [ -z "${RUSTUP_HOME:-}" ] && [ -n "${CARGO_HOME:-}" ]; then
  _ru=$(CDPATH=; cd "$CARGO_HOME/.." && pwd)/.rustup
  [ -d "$_ru" ] && { RUSTUP_HOME=$_ru; export RUSTUP_HOME; }
fi
unset _cb _ru
