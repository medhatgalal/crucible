#!/bin/sh
# Prove the release package is reproducible, minimal, executable, and cold-start capable.
set -eu

ROOT=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(cat "$ROOT/VERSION")
REF=${1:-HEAD}
TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-package.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$TMP"' 0
trap 'rm -rf "$TMP"; exit 129' 1
trap 'rm -rf "$TMP"; exit 130' 2
trap 'rm -rf "$TMP"; exit 143' 15

"$ROOT/scripts/package-release.sh" "$VERSION" "$REF" "$TMP/one" >/dev/null
"$ROOT/scripts/package-release.sh" "$VERSION" "$REF" "$TMP/two" >/dev/null
NAME="crucible-$VERSION.tar.gz"
cmp "$TMP/one/$NAME" "$TMP/two/$NAME"
cmp "$TMP/one/$NAME.sha256" "$TMP/two/$NAME.sha256"

CONTENTS=$(tar -tzf "$TMP/one/$NAME")
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/BOOTSTRAP.md$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/crucible$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/scripts/verify-agent-cycle.sh$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/scripts/verify-coldstart-independence.sh$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/roles/contract-auditor.md$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/docs/drive.md$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/docs/install.md$"
printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/scripts/verify-drive.sh$"
if printf '%s\n' "$CONTENTS" | grep -Eq "^crucible-$VERSION/(reports|\.github|dist)/"; then
  echo "verify-package: package contains development-only paths" >&2; exit 1
fi

mkdir "$TMP/extract"
tar -xzf "$TMP/one/$NAME" -C "$TMP/extract"
PACKAGE="$TMP/extract/crucible-$VERSION"
[ -x "$PACKAGE/crucible" ]
[ -x "$PACKAGE/scripts/verify-agent-cycle.sh" ]
[ -x "$PACKAGE/scripts/verify-coldstart-independence.sh" ]
"$PACKAGE/crucible" help >/dev/null
"$PACKAGE/scripts/verify-agent-cycle.sh" >/dev/null
"$PACKAGE/scripts/verify-coldstart-independence.sh" >/dev/null
[ -x "$PACKAGE/scripts/verify-drive.sh" ]
"$PACKAGE/scripts/verify-drive.sh" >/dev/null

printf 'PACKAGE-OK %s %s\n' "$VERSION" "$(awk '{print $1}' "$TMP/one/$NAME.sha256")"
