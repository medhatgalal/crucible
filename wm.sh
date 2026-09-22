#!/bin/sh
set -eu
bindir=$(CDPATH=; cd -- "$(dirname -- "$0")" && pwd)
WM_WRAPPER=$0; export WM_WRAPPER
exec "$bindir/crucible" "$@"
