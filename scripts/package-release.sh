#!/bin/sh
# Build one host package: git archive of sources plus the release `crucible` binary.
set -eu

ROOT=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
VERSION_ARG=${1:-$(cat "$ROOT/VERSION")}
REF=${2:-HEAD}
OUT_ARG=${3:-$ROOT/dist}

if ! printf '%s\n' "$VERSION_ARG" | awk -F. '
  NF != 3 { exit 1 }
  { for (i = 1; i <= 3; i++) if ($i !~ /^[0-9]+$/) exit 1 }
'; then
  echo "package-release: version must be MAJOR.MINOR.PATCH: $VERSION_ARG" >&2
  exit 2
fi
git -C "$ROOT" rev-parse --verify "$REF^{commit}" >/dev/null 2>&1 \
  || { echo "package-release: no commit for ref: $REF" >&2; exit 2; }
RECORDED=$(git -C "$ROOT" show "$REF:VERSION" 2>/dev/null) \
  || { echo "package-release: $REF has no VERSION" >&2; exit 2; }
[ "$RECORDED" = "$VERSION_ARG" ] \
  || { echo "package-release: requested $VERSION_ARG but $REF records $RECORDED" >&2; exit 2; }

case $OUT_ARG in /*) OUT=$OUT_ARG ;; *) OUT=$PWD/$OUT_ARG ;; esac
mkdir -p "$OUT"
NAME="crucible-$VERSION_ARG.tar.gz"
PREFIX="crucible-$VERSION_ARG"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/crucible-pkg.XXXXXX")
TMP="$OUT/.$NAME.$$.tmp"
# Cleanup must never mask the exit status.
trap 'rm -rf "$STAGE"; rm -f "$TMP"' 0
trap 'rm -rf "$STAGE"; rm -f "$TMP"; exit 129' 1
trap 'rm -rf "$STAGE"; rm -f "$TMP"; exit 130' 2
trap 'rm -rf "$STAGE"; rm -f "$TMP"; exit 143' 15

git -C "$ROOT" archive --format=tar --prefix="$PREFIX/" "$REF" | tar -x -C "$STAGE"
PKG="$STAGE/$PREFIX"
[ -d "$PKG" ] || { echo "package-release: archive missing $PREFIX/" >&2; exit 2; }

# Host-built product binary from the archived sources (matches REF).
# shellcheck disable=SC1091
. "$ROOT/scripts/cargo-env.sh"
if ! ( CDPATH=; cd -- "$PKG" && cargo build --release --locked ); then
  echo "package-release: cargo build --release failed" >&2
  exit 2
fi
RUST_BIN="$PKG/target/release/crucible"
[ -x "$RUST_BIN" ] || { echo "package-release: missing $RUST_BIN" >&2; exit 2; }

# crucible-guided is the committed sibling exec. Do not copy the finder
# wrapper over it. The packaged crucible name is the host-built binary.
cp "$RUST_BIN" "$PKG/crucible"
chmod +x "$PKG/crucible"
rm -rf "$PKG/target"

# Working-mode payload (9c): if the ref tracks these paths, they must land in the tarball.
CONTENTS=$( ( CDPATH=; cd -- "$STAGE" && tar -cf - "$PREFIX" ) | tar -t )
for rel in wm.sh skills/architecture/SKILL.md ROUTING.tsv \
  adapters/grok.md adapters/claude.md adapters/codex.md adapters/kiro.md; do
  if git -C "$ROOT" cat-file -e "$REF:$rel" 2>/dev/null; then
    printf '%s\n' "$CONTENTS" | grep -q "^$PREFIX/$rel\$" \
      || { echo "package-release: $REF has $rel but archive does not" >&2; exit 2; }
  fi
done
printf '%s\n' "$CONTENTS" | grep -q "^$PREFIX/crucible\$" \
  || { echo "package-release: missing $PREFIX/crucible" >&2; exit 2; }
printf '%s\n' "$CONTENTS" | grep -q "^$PREFIX/wm.sh\$" \
  || { echo "package-release: missing $PREFIX/wm.sh" >&2; exit 2; }
printf '%s\n' "$CONTENTS" | grep -q "^$PREFIX/.grok/rules/loop-router.md\$" \
  || { echo "package-release: missing $PREFIX/.grok/rules/loop-router.md" >&2; exit 2; }
printf '%s\n' "$CONTENTS" | grep -q "^$PREFIX/templates/herdr/workspace\$" \
  || { echo "package-release: missing $PREFIX/templates/herdr/workspace" >&2; exit 2; }
printf '%s\n' "$CONTENTS" | grep -q "^$PREFIX/templates/herdr/roles\$" \
  || { echo "package-release: missing $PREFIX/templates/herdr/roles" >&2; exit 2; }

# Reproducible gzip of the staged tree (binary is host-built).
( CDPATH=; cd -- "$STAGE" && tar -cf - "$PREFIX" ) | gzip -n -9 > "$TMP"
[ -s "$TMP" ] || { echo "package-release: empty package" >&2; exit 2; }

if [ -f "$OUT/$NAME" ]; then
  cmp -s "$TMP" "$OUT/$NAME" \
    || { echo "package-release: existing $OUT/$NAME differs; refusing overwrite" >&2; exit 2; }
  rm -f "$TMP"
else
  mv "$TMP" "$OUT/$NAME"
fi
trap 'rm -rf "$STAGE"' 0
trap 'rm -rf "$STAGE"; exit 129' 1
trap 'rm -rf "$STAGE"; exit 130' 2
trap 'rm -rf "$STAGE"; exit 143' 15

if command -v shasum >/dev/null 2>&1; then
  HASH=$(shasum -a 256 "$OUT/$NAME" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  HASH=$(sha256sum "$OUT/$NAME" | awk '{print $1}')
else
  echo "package-release: need shasum or sha256sum" >&2; exit 2
fi
printf '%s  %s\n' "$HASH" "$NAME" > "$OUT/$NAME.sha256"
printf '%s\n%s\n' "$OUT/$NAME" "$OUT/$NAME.sha256"
