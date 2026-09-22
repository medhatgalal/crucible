#!/bin/sh
# Prove the release package is minimal, executable, and cold-start capable.
# POSIX/source members are byte-compared across two builds; the host-built
# `crucible` binary is sha256'd separately (not cmp'd as part of the tarball).
set -eu

ROOT=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(cat "$ROOT/VERSION")
REF=${1:-HEAD}

ROOT_REAL=$(unset CDPATH; cd -- "$ROOT" && pwd -P)
if ! TOP=$(git -C "$ROOT" rev-parse --show-toplevel 2>/dev/null) \
  || [ "$(unset CDPATH; cd -- "$TOP" && pwd -P)" != "$ROOT_REAL" ]; then
  printf 'verify-package: %s is not the root of a git repository, so the packaging assertions are skipped (every assertion needs a commit of THIS tree to archive; this script ships inside the release package, which has no object database of its own even when it is unpacked inside another repository, and no ref argument can supply one — inside a checkout every assertion still runs and still refuses)\n' "$ROOT"
  exit 0
fi

TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-package.XXXXXX")
trap 'rm -rf "$TMP"' 0
trap 'rm -rf "$TMP"; exit 129' 1
trap 'rm -rf "$TMP"; exit 130' 2
trap 'rm -rf "$TMP"; exit 143' 15

package_has_whats_new() { [ -f "$1/docs/whats-new.md" ]; }
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

sha256_file() {
  f=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    openssl dgst -sha256 "$f" | awk '{print $NF}'
  fi
}

mkdir "$TMP/extract-one" "$TMP/extract-two"
tar -xzf "$TMP/one/$NAME" -C "$TMP/extract-one"
tar -xzf "$TMP/two/$NAME" -C "$TMP/extract-two"
ONE="$TMP/extract-one/crucible-$VERSION"
TWO="$TMP/extract-two/crucible-$VERSION"
[ -f "$ONE/crucible" ] || { echo "verify-package: missing crucible binary" >&2; exit 1; }
[ -f "$TWO/crucible" ] || { echo "verify-package: missing crucible binary in second package" >&2; exit 1; }

# POSIX/source members must match; do not cmp the host-built binary.
( CDPATH=; cd -- "$ONE" && find . -type f ! -path './crucible' | sort ) > "$TMP/list-one"
( CDPATH=; cd -- "$TWO" && find . -type f ! -path './crucible' | sort ) > "$TMP/list-two"
cmp "$TMP/list-one" "$TMP/list-two" \
  || { echo "verify-package: POSIX member lists differ" >&2; exit 1; }
while IFS= read -r rel; do
  cmp "$ONE/$rel" "$TWO/$rel" \
    || { echo "verify-package: POSIX member differs: $rel" >&2; exit 1; }
done < "$TMP/list-one"

BIN_HASH=$(sha256_file "$ONE/crucible")
printf 'verify-package: crucible binary sha256 %s\n' "$BIN_HASH"

CONTENTS=$(tar -tzf "$TMP/one/$NAME")
for REQUIRED in \
  BOOTSTRAP.md \
  crucible \
  wm.sh \
  WORKING-MODE.md \
  ROUTING.tsv \
  scripts/project-skills.sh \
  scripts/verify-agent-cycle.sh \
  scripts/verify-coldstart-independence.sh \
  roles/contract-auditor.md \
  docs/drive.md \
  docs/install.md \
  docs/whats-new.md \
  scripts/verify-drive.sh \
  skills/architecture/SKILL.md \
  skills/critique/SKILL.md \
  skills/review/SKILL.md \
  skills/loop-design/SKILL.md \
  skills/research/SKILL.md \
  skills/working-mode/SKILL.md \
  adapters/grok.md \
  adapters/claude.md \
  adapters/codex.md \
  adapters/kiro.md
do
  printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/$REQUIRED\$" \
    || { echo "verify-package: package is missing crucible-$VERSION/$REQUIRED" >&2; exit 1; }
done
if printf '%s\n' "$CONTENTS" | grep -Eq "^crucible-$VERSION/(reports|\.github|dist)/"; then
  echo "verify-package: package contains development-only paths" >&2; exit 1
fi
if printf '%s\n' "$CONTENTS" | grep -Eq "^crucible-$VERSION/target/"; then
  echo "verify-package: package contains cargo target/" >&2; exit 1
fi

PACKAGE="$ONE"
package_has_whats_new "$PACKAGE" \
  || { echo "verify-package: extracted package is missing docs/whats-new.md" >&2; exit 1; }
[ -x "$PACKAGE/crucible" ]
[ -x "$PACKAGE/wm.sh" ]
[ -x "$PACKAGE/scripts/verify-agent-cycle.sh" ]
[ -x "$PACKAGE/scripts/verify-coldstart-independence.sh" ]
_sig=$(dd if="$PACKAGE/crucible" bs=2 count=1 2>/dev/null || true)
[ "$_sig" != '#!' ] \
  || { echo "verify-package: crucible must be the host-built binary, not a script" >&2; exit 1; }
"$PACKAGE/crucible" --version | grep -q "$VERSION" \
  || { echo "verify-package: crucible --version is not $VERSION" >&2; exit 1; }
"$PACKAGE/crucible" help >/dev/null
sh -n "$PACKAGE/wm.sh"
"$PACKAGE/scripts/verify-agent-cycle.sh" >/dev/null
"$PACKAGE/scripts/verify-coldstart-independence.sh" >/dev/null
[ -x "$PACKAGE/scripts/verify-drive.sh" ]
"$PACKAGE/scripts/verify-drive.sh" >/dev/null

printf 'PACKAGE-OK %s %s bin=%s\n' "$VERSION" "$(awk '{print $1}' "$TMP/one/$NAME.sha256")" "$BIN_HASH"
