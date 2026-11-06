# Building and repairing releases

A semantic tag such as `v0.0.2-alpha` triggers the four-target release workflow:
Linux x86_64, Windows x86_64, macOS arm64 and macOS x86_64. Each crate version
must match the tag after removing its leading `v`. The publish job runs only
after all build/package jobs succeed. Alpha tags are GitHub prereleases.

## Retry an existing tag without changing its source

A packaging or workflow correction can be committed to `main`, then used to
retry an existing source tag:

```sh
gh workflow run release.yml --ref main -f release_tag=v0.0.2-alpha
```

The workflow definition comes from the selected workflow revision. Every binary
build and the Cargo dependency inventory use a checkout of the requested tag.
Linux packaging uses the helper from the workflow revision in a separate sparse
checkout. This repairs packaging without force-moving a published tag or silently
building binaries from newer application code. The crate/tag version guard still
runs. Supply an existing semantic tag, not a branch name.

The published `release-provenance.json` records the tag, source commit, packaging
workflow commit and workflow run ID. `SHA256SUMS.txt` covers that provenance file,
the Cargo metadata dependency inventory and every downloadable binary/package.
These are checksums and an inventory, not a signed attestation or code signature.

## Debian prerelease regression

The first `v0.0.2-alpha` workflow compiled all four platform binaries. Its Linux
package step failed because the replacement in Bash `${VERSION/-/~}` expanded
the tilde to a home path under the runner's Bash, producing an invalid `sed`
expression. The helper now converts the first hyphen with a literal sed
replacement, so `0.0.2-alpha` becomes `0.0.2~alpha`. It validates the version and
architecture before touching output directories, requires both binaries and
uses `dpkg-deb --root-owner-group`. Linux CI checks the archive's ownership.

Automated packaging tests run the actual helper in isolated directories with
explicit fixture payloads. They verify prerelease/stable versions, architecture
normalization, both installed paths, and rejection of invalid/missing inputs.
These assembly tests do not claim that fixture payloads are executable Linux
programs; the native CI build supplies the actual release binaries.

## Release evidence

- Source tag `v0.0.2-alpha`: `b43084cc0add3e12f379d6a62521712aa57ee27c`.
- Initial workflow: [42234295796](https://github.com/MJKhaani/Shellumi/actions/runs/42234295796).
  All binary builds passed; Linux packaging failed, so publishing was skipped.
- The packaging repair adds three regression tests on main (321 workspace tests
  in total); the source tag's application suite has 318 tests. No application
  source is changed by this packaging repair.
- Retry/publication status must be checked on GitHub Actions before treating
  release assets as available. Runtime acceptance on each OS remains separate
  from successful compilation and packaging.
