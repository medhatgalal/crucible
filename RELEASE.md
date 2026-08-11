# Release procedure

Prepare the version and changelog on a review branch. Publish only the reviewed commit after it is
merged to a clean, synchronized `main`.

## Release candidate

```sh
version=$(cat VERSION)
grep -q "^## \[$version\] - $(date +%F)$" CHANGELOG.md
git diff --check
./scripts/selftest.sh -v
./scripts/verify-agent-cycle.sh
./scripts/verify-package.sh
```

Open a pull request, wait for its exact-head checks, and merge it. Do not tag a branch commit or a
locally different tree.

## Publish the merged commit

From a clean, synchronized `main`:

```sh
version=$(cat VERSION)
test "$(git branch --show-current)" = main
test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"
git tag -a "v$version" -m "Crucible $version"
test "$(git show "v$version:VERSION")" = "$version"
git push origin "v$version"
```

Wait for both Linux and macOS tag jobs at that exact SHA. Then build from the immutable tag and publish
the two assets:

```sh
./scripts/package-release.sh "$version" "v$version" dist
gh release create "v$version" \
  "dist/crucible-$version.tar.gz" \
  "dist/crucible-$version.tar.gz.sha256" \
  --title "Crucible $version" --generate-notes --verify-tag
```

Download the published assets into a temporary directory and verify their checksum before calling the
release complete.

## Manual mutation evidence

When a release changes `cmd_close` or judge-verdict archival, test the changed guard on a disposable
copy by removing it and proving the expected refusal fails. Record the exact mutation and result in the
pull request. These targeted checks are not required when neither mechanism changed.
