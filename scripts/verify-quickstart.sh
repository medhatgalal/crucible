#!/bin/sh
# Compatibility entrypoint. The quickstart is now the cold fresh-agent cycle, not an operator recipe.
set -eu
HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")" && pwd)
exec "$HERE/verify-agent-cycle.sh"
