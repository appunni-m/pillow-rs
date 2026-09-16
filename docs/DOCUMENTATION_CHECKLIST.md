# Maintaining the documentation

Documentation is a public interface. Keep installation, examples, support
limits, benchmark interpretation, and contributor commands usable from a
published package or a fresh checkout.

## Sources and audiences

| Audience | Start | Authoritative detail |
| --- | --- | --- |
| Python user | [Installation](INSTALLATION.md), [recipes](PYTHON.md) | Public manifest and executed parity inputs |
| Rust user | [Rust integration](RUST.md) | Versioned rustdoc and Cargo feature definitions |
| Node/browser user | [JavaScript guide](../pillow-rs-js/README.md) | Package exports and generated declarations |
| Evaluator | [Maturity](COMPATIBILITY.md), [benchmarks](BENCHMARKING.md) | Source-bound, dated execution evidence |
| Contributor | [Contributing](../CONTRIBUTING.md), [commands](COMMANDS.md) | Makefile, generators, and source map |
| Maintainer | [Releasing](../RELEASING.md) | Pinned GitHub workflows and registry artifact identity |

`documentation.json` selects the pages published from this repository.
`mkdocs.yml` owns navigation. The site builds from reviewed Markdown and
validated evidence into `target/site/`; generated HTML is not committed.

## Change checklist

- [ ] Identify the audience and a concrete task before adding a page.
- [ ] Keep one canonical explanation; link to it instead of copying a report.
- [ ] Verify installation names, versions, platform requirements, and examples.
- [ ] Distinguish declared, tested, partial, unsupported, and unmeasured scope.
- [ ] State the source revision, runner, denominator, and limitations for measurements.
- [ ] Keep failed results and absent measurements visible.
- [ ] Check links, headings, commands, package contents, and rendered pages.
- [ ] Preserve licenses, fixture provenance, and the final Puhu/Pillow acknowledgements.
- [ ] Remove superseded internal plans and audit diaries after consolidating durable guidance.

Old measurements remain historical. Regenerating a presentation does not create
a new coverage receipt or benchmark. Public benchmark snapshots retain the
original report hash and omit local machine paths and hostnames.

## Validate and preview

```sh
make docs-setup
make docs-test docs-lint
make docs-examples
make docs-build
make docs-serve
```

The build fails on missing pages, invalid local links and anchors, missing
assets, retired Python names, malformed benchmark snapshots, and attribution
regressions. It also checks rendered HTML. Inspect the site with a narrow
viewport, keyboard navigation, light/dark themes, and search before merging
navigation or styling changes.

The documentation dependencies are fully pinned with hashes in
`requirements-docs.txt`. Update `requirements-docs.in`, run
`make docs-lock`, review the lock change, and repeat validation.

## Publish from this repository

The Documentation workflow builds pull requests without deploying. Main pushes
and manual runs publish the checked artifact through GitHub Pages using
`github-pages`. The Pages source must be GitHub Actions in this repository's
settings. No separate site repository, registry token, or generated-content
branch is required.

Benchmark workflow artifacts can refresh the public result page. Their data is
validated against this repository and the measured commit before rendering;
downloaded data never supplies executable site code. The checked-in snapshot
remains a reproducible fallback for documentation builds.
