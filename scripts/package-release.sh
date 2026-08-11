#!/bin/sh
# Build one reproducible, self-contained Crucible source package from a Git ref.
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
TMP="$OUT/.$NAME.$$.tmp"
trap 'rm -f "$TMP"' 0 1 2 15

git -C "$ROOT" archive --format=tar --prefix="crucible-$VERSION_ARG/" "$REF" | gzip -n -9 > "$TMP"
[ -s "$TMP" ] || { echo "package-release: empty package" >&2; exit 2; }

if [ -f "$OUT/$NAME" ]; then
  cmp -s "$TMP" "$OUT/$NAME" \
    || { echo "package-release: existing $OUT/$NAME differs; refusing overwrite" >&2; exit 2; }
  rm -f "$TMP"
else
  mv "$TMP" "$OUT/$NAME"
fi
trap - 0 1 2 15

if command -v shasum >/dev/null 2>&1; then
  HASH=$(shasum -a 256 "$OUT/$NAME" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  HASH=$(sha256sum "$OUT/$NAME" | awk '{print $1}')
else
  echo "package-release: need shasum or sha256sum" >&2; exit 2
fi
printf '%s  %s\n' "$HASH" "$NAME" > "$OUT/$NAME.sha256"
printf '%s\n%s\n' "$OUT/$NAME" "$OUT/$NAME.sha256"
