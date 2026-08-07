# Release procedure

Run these commands from a clean checkout on the branch to release. Replace `0.1.1` with the intended version.

## 1. Bump `VERSION`

```sh
version=0.1.1
printf '%s\n' "$version" > VERSION
test "$(cat VERSION)" = "$version"
```

## 2. Update `CHANGELOG.md` with a dated release entry

```sh
"${EDITOR:-vi}" CHANGELOG.md
grep -q "^## \[$version\] - $(date +%F)$" CHANGELOG.md
git diff --check
git add VERSION CHANGELOG.md
git commit -m "Release $version"
```

## 3. Tag the release

```sh
git tag -a "v$version" -m "Release $version"
```

## 4. Push the commit and tag

```sh
git push origin HEAD
git push origin "v$version"
```

## 5. Verify the tag contains the same version as `VERSION`

```sh
test "$(git show "v$version:VERSION")" = "$version"
printf 'verified v%s contains VERSION=%s\n' "$version" "$version"
```

## Mutation checks before a release

Two properties cannot be asserted reliably by the suite, because two guards overlap and which one
fires is a race. Verify them by hand, on a copy, and restore afterwards:

1. **The close-time work-id comparator.** Remove the `[ "$before" = "$after" ]` guard in `cmd_close`.
   Then build an item with valid work, evidence and two passing verdicts, mutate the work while
   `close` is running, and confirm it **closes at the new work id**. Restore, and confirm it refuses.
   If the mutated build still refuses, the comparator is redundant and the claim in `README.md`
   should be rewritten rather than kept.

2. **The verdict archive.** Remove the `mv "$old" "$a"` in the judge branch of `cmd_dispatch`, then
   dispatch a judge onto changed work and confirm the superseded verdict is lost. Restore, and
   confirm it lands under `verdicts/history/`.

Record both results in the release commit message. A release whose mutation checks were not run is
a release whose two weakest assertions are unverified.
