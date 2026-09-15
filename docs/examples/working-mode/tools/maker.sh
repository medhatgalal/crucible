#!/bin/sh
# Fixture maker for the LOW hello-world example (not a live harness).
set -eu
mkdir -p .wm product
sha_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}
if [ ! -f .wm/FALSIFIER ]; then
  printf 'grep -qx hello product/hello.txt\n' > .wm/FALSIFIER
  h=$(sha_of .wm/FALSIFIER)
  wid=NOCOMMIT
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    wid=$(git rev-parse --short=12 HEAD)
  fi
  printf 'agent: carol\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
  exit 0
fi
printf 'hello\n' > product/hello.txt
git add product/hello.txt
git commit -qm 'maker-build hello'
