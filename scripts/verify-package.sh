#!/bin/sh
# Prove the release package is reproducible, minimal, executable, and cold-start capable.
set -eu

ROOT=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(cat "$ROOT/VERSION")
REF=${1:-HEAD}

# Assert if this tree is the repository, skip if it is not, and the asymmetry is the point.
#
# Every assertion runs through package-release.sh, which archives a commit, so the suite needs THIS
# tree's object database. The script also ships inside the release package, which has none and which
# no `REF` argument can rescue, so demanding a commit unconditionally made it fail in every tree but
# a maintainer checkout for a reason that was not about the tree it ran in. Presence is unchanged:
# inside a checkout a genuinely broken package still refuses.
#
# The test is that `$ROOT` IS a repository's top level, not that some ancestor is one, because
# `rev-parse` searches upward: a package unpacked inside an existing repo — the likeliest install
# location — would otherwise be checked against the enclosing objects and report `HEAD has no
# VERSION`, the exact false alarm this guard removes. Both paths go through `pwd -P` because on
# macOS `/var` is a symlink to `/private/var` and a textual compare would misread a checkout.
#
# Not a `.git` path test: a linked worktree keeps a `.git` FILE, and `--show-toplevel` reports that
# worktree's own root, so the suite still runs there where a path test would skip it — a silent
# pass, the one failure mode a verification script must never have. Git absent from the machine
# fails the command and skips too, correctly: without git there is no commit to archive.
ROOT_REAL=$(unset CDPATH; cd -- "$ROOT" && pwd -P)
if ! TOP=$(git -C "$ROOT" rev-parse --show-toplevel 2>/dev/null) \
  || [ "$(unset CDPATH; cd -- "$TOP" && pwd -P)" != "$ROOT_REAL" ]; then
  printf 'verify-package: %s is not the root of a git repository, so the packaging assertions are skipped (every assertion needs a commit of THIS tree to archive; this script ships inside the release package, which has no object database of its own even when it is unpacked inside another repository, and no ref argument can supply one — inside a checkout every assertion still runs and still refuses)\n' "$ROOT"
  exit 0
fi

TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-package.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$TMP"' 0
trap 'rm -rf "$TMP"; exit 129' 1
trap 'rm -rf "$TMP"; exit 130' 2
trap 'rm -rf "$TMP"; exit 143' 15

package_has_whats_new() { [ -f "$1/docs/whats-new.md" ]; }
# Mutation-test the extracted-path predicate before real archive checks, so the proof still runs
# when an intentionally incomplete package produces the expected red result below.
PACKAGE_FIXTURE="$TMP/package-fixture"
mkdir -p "$PACKAGE_FIXTURE/docs"
printf '# fixture\n' > "$PACKAGE_FIXTURE/docs/whats-new.md"
package_has_whats_new "$PACKAGE_FIXTURE" \
  || { echo "verify-package: whats-new package predicate rejects its complete fixture" >&2; exit 1; }
mv "$PACKAGE_FIXTURE/docs/whats-new.md" "$PACKAGE_FIXTURE/docs/whats-new.mutated"
if package_has_whats_new "$PACKAGE_FIXTURE"; then
  echo "verify-package: whats-new package predicate accepted an isolated missing-file mutation" >&2
  exit 1
fi

"$ROOT/scripts/package-release.sh" "$VERSION" "$REF" "$TMP/one" >/dev/null
"$ROOT/scripts/package-release.sh" "$VERSION" "$REF" "$TMP/two" >/dev/null
NAME="crucible-$VERSION.tar.gz"
cmp "$TMP/one/$NAME" "$TMP/two/$NAME"
cmp "$TMP/one/$NAME.sha256" "$TMP/two/$NAME.sha256"

CONTENTS=$(tar -tzf "$TMP/one/$NAME")
# One anchored pattern per required path, as before, but naming the one that is absent. A bare
# `grep -q` under `set -e` exits 1 with nothing on stderr, so a genuine packaging failure inside a
# checkout refused silently and the reader had to bisect the file to learn what was dropped. Every
# previous path and anchored pattern remains; docs/whats-new.md adds the travelling release note.
for REQUIRED in \
  BOOTSTRAP.md \
  crucible \
  scripts/verify-agent-cycle.sh \
  scripts/verify-coldstart-independence.sh \
  roles/contract-auditor.md \
  docs/drive.md \
  docs/install.md \
  docs/whats-new.md \
  scripts/verify-drive.sh
do
  printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/$REQUIRED\$" \
    || { echo "verify-package: package is missing crucible-$VERSION/$REQUIRED" >&2; exit 1; }
done
if printf '%s\n' "$CONTENTS" | grep -Eq "^crucible-$VERSION/(reports|\.github|dist)/"; then
  echo "verify-package: package contains development-only paths" >&2; exit 1
fi

mkdir "$TMP/extract"
tar -xzf "$TMP/one/$NAME" -C "$TMP/extract"
PACKAGE="$TMP/extract/crucible-$VERSION"
package_has_whats_new "$PACKAGE" \
  || { echo "verify-package: extracted package is missing docs/whats-new.md" >&2; exit 1; }
[ -x "$PACKAGE/crucible" ]
[ -x "$PACKAGE/scripts/verify-agent-cycle.sh" ]
[ -x "$PACKAGE/scripts/verify-coldstart-independence.sh" ]
"$PACKAGE/crucible" help >/dev/null
"$PACKAGE/scripts/verify-agent-cycle.sh" >/dev/null
"$PACKAGE/scripts/verify-coldstart-independence.sh" >/dev/null
[ -x "$PACKAGE/scripts/verify-drive.sh" ]
"$PACKAGE/scripts/verify-drive.sh" >/dev/null

printf 'PACKAGE-OK %s %s\n' "$VERSION" "$(awk '{print $1}' "$TMP/one/$NAME.sha256")"
