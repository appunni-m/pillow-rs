# Documentation maintenance

User documentation answers: how do I install the package, use it, interpret
results, and understand its limitations? Contributor documentation explains
source builds, tests, fixtures, benchmark collection, and release procedures.

## Required review

- [ ] Install from the appropriate package manager; keep source setup under Contribute.
- [ ] Lead benchmark pages with results, units, and a clear current/historical label.
- [ ] Keep supported and unsupported features concise; link detailed contracts separately.
- [ ] Verify examples against the published package, not just the workspace source.
- [ ] Keep package names, release links, API references, and platform requirements accurate.
- [ ] Preserve historical measurements, source revisions, and incomplete results.
- [ ] Keep attribution, licenses, and the final Puhu/Pillow acknowledgements.
- [ ] Preview the site at narrow and wide widths; check search and navigation.

## Release freshness

`documentation.json` records the published release separately from the source
version. Each page declares an audience. User pages must have a generated
release reference and must not contain repository test/build instructions.
Paired `release:*` comments keep installation blocks synchronized with the
published record. Keep these markers when editing Markdown.

After a release has finished publishing to its registries:

```sh
make docs-release-refresh
make docs-release-check
make docs-registry-examples
```

Review and commit the changed release record and user pages. The refresh
checks the newest published GitHub release (including alphas), resolves its
tag to the immutable commit, and verifies the matching registry versions.
It does not bump source versions or rewrite old benchmark and coverage data.
An API error fails the check; it does not silently accept stale information.

The Documentation workflow checks release freshness on main updates, successful tag releases, manual
runs, and daily. Pull requests run offline structure checks and example checks;
release freshness is verified before deploying main. A pending source version
may differ from the published version without directing users to unavailable packages.

## Build and validate

```sh
make docs-setup
make docs-test docs-lint
make docs-examples
make docs-build
make docs-serve
```

The offline checks validate audience assignments, managed release blocks,
installation and API pins, relative Markdown links, rendered links and anchors
(including absolute links to this project's Pages site), and final attribution.
They cannot prove every prose claim or every external page's contents; review
behavioral claims against the implementation and recorded evidence.

`make docs-registry-examples` creates temporary consumer projects, installs exact
published versions, and executes the Markdown quickstarts. Rust compilation
is cached under `target/docs-registry-cargo`; temporary environments are removed.
The source example check remains separate so a future source change cannot
hide a broken published-package example.

Documentation tools use the hash-locked `requirements-docs.txt`. Update the
direct pins and use `make docs-lock` for intentional tool upgrades.

## Publish and benchmark data

Each repository deploys its own site to GitHub Pages through Actions.
`mkdocs.yml` owns navigation, `documentation.json` selects source pages, and
`target/site` is generated output. Successful trusted main benchmark runs can
supply the displayed data. Only data is imported from benchmark artifacts;
executable site code comes from the reviewed checkout.

Results show all recorded rows, including failures, fallbacks, and unmeasured
values. Detailed source hashes, environment, sample boundaries, and policies
are available under Contribute. A documentation build never reruns a benchmark
or turns an older measurement into evidence for a new release.

## Browser review

With the JavaScript development dependencies installed, run
`make docs-browser-check` to check result filtering and desktop/mobile layout.
The check starts a temporary loopback server and saves review screenshots under
`target/docs-preview/`. For an already running preview, pass its benchmark URL
through `DOCS_BROWSER_URLS`.
