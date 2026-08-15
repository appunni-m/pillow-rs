# Tiny Image Star integration issues — solution proposal

Status: proposal only. This document records review findings and a recommended
implementation sequence; it does not change the binding, codec, packaging, or
dependency code.

## Review basis

The attached issue note was captured at `5fdf6db8` (`Cover merge band-count and
composite mode paths`). That commit is an ancestor of the current checkout,
which is `99f4dddf` (`Cover JPEG EXIF scanner end-of-input parity`) as reviewed
on 2026-08-02. The tracked worktree was clean during review; generated
`pillow-rs-js/pkg` files exist but are ignored.

The most important verification was:

```text
CARGO_TARGET_DIR=/private/tmp/pillow-rs-review-target \
  cargo check -p pillow-rs-js --target wasm32-unknown-unknown --locked --offline
```

It reaches `pillow-rs-js` and fails on the same four core/binding API drifts
listed in the issue note. A package dry-run also fails before packaging because
`fontdone` is a path-only dependency with no version requirement.

The primary source locations are the [WASM binding](../pillow-rs-js/src/lib.rs),
the [core image API](../pillow-rs/src/image.rs), the [core feature table](../pillow-rs/Cargo.toml),
the [JS feature table](../pillow-rs-js/Cargo.toml), the [npm manifest](../pillow-rs-js/package.json),
and the [release target](../Makefile).

## Executive recommendation

Treat the work as four ordered boundaries:

1. Restore a compiling, version-matched WASM binding.
2. Make the Rust and npm release inputs independent of sibling checkouts.
3. Expose an explicit container-encoding API and a capability-driven format
   contract.
4. Add interactive cancellation, metadata, and animation only after their
   browser contracts are defined and tested.

For the immediate Tiny Image Star integration, ship still-image PNG plus the
formats that pass the WASM fixture matrix, keep AVIF output disabled, run jobs
in a Worker, and discard results whose revision is no longer current. Do not
claim that Worker isolation is cancellation: a synchronous WASM call already
running in a Worker cannot be interrupted by ordinary event-loop code.

## Finding review

| Area | Status at current checkout | Priority | Proposed direction |
| --- | --- | --- | --- |
| JS binding API drift | Confirmed. `quantize`, `reduce`, `colorize`, and `getColor` no longer match the core signatures or return types. | P0 | Synchronize the binding adapter with the core contract, then make both WASM feature lanes compile in CI before producing packages. |
| Format-selectable output | Confirmed. Core has `Image::encode(format)`, while WASM `save()` is zero-argument PNG and `toBytesEncoded()` is the raw-byte encoder. | P0 | Add a distinct `encode(format, options)` binding surface; keep `save()` as a documented PNG compatibility alias only if existing consumers require it. |
| AVIF | Confirmed capability gap, not solved by enabling a Cargo feature. The locked `image-slash-star` revision describes AVIF as native-only; its WASM encoder path is unavailable. | P0 if the product requires AVIF; otherwise P1 | Keep AVIF out of the browser promise. If full browser AVIF is required, first land and pin a WASM-capable codec, then add a separately tested `wasm-avif` lane. |
| WASM core/extra split | Confirmed documentation/manifest mismatch. The packaging record describes PNG-only `core` plus codec-rich `extra`, but the current JS features map `wasm-core`, `wasm-extra`, and `wasm-all` to the same `image-codecs-all` surface. | P1 | Either restore a real PNG-only/core versus extra split, or remove the split and document one complete artifact. Rebuild the capability, size, and fixture matrices from the chosen contract. |
| Progress/cancellation | Confirmed binding gap. The locked codec revision has no public cancellation token surface; the newer sibling checkout does, but it is not the revision in `Cargo.lock` and is not threaded through `pillow-rs`. | P1 | Use Worker revision fencing now. Later thread a pinned codec token through core materialization/encoding and expose cancellation as cooperative, with documented checkpoints and limits. |
| npm identity | Confirmed contract mismatch. `package.json` says `pillow-rs`; README and CONTRIBUTING say `@pillow-rs/wasm`; generated variant manifests say `pillow-rs-js`. | P1 | Select one public npm name and one import contract. The existing scoped documentation suggests `@pillow-rs/wasm`, subject to npm name availability; otherwise update all documentation to `pillow-rs`. |
| npm release location | Confirmed release-script defect. `make release-npm` publishes from `pillow-rs-js/pkg`, but that directory has no root `package.json`; the publishable manifest is at `pillow-rs-js/package.json`. | P1 | Build both variants, validate from the JS package root, and publish that root or an explicit staging directory. |
| `fontdone` source dependency | Confirmed. `pillow-rs/Cargo.toml` references `../../fontdone`; a clean checkout cannot resolve it and `cargo package` rejects the missing version requirement. | P1 | Publish a versioned `fontdone` crate and use a registry requirement for release builds. Keep sibling development through a local, uncommitted Cargo patch, or use a pinned source distribution until the registry release exists. |
| Ignored generated `pkg` | Not intrinsically a defect. Ignoring generated WASM is safe when CI or npm publication regenerates it; it is a defect if the app expects the directory to exist after cloning the source repository. | P1 conditional | Make the app consume a versioned npm artifact, or make the CI artifact handoff explicit. Do not make a local ignored directory the app's undocumented source dependency. |
| Initialization | Confirmed usability risk. The generated web module has async default initialization; `Image.open(bytes)` is synchronous only after initialization. | P2 | Hide initialization in one app adapter with a single `ready()` promise, then expose synchronous image operations behind that boundary. |
| Animation and metadata | Confirmed incomplete. Core child-image iteration is documented as empty; the JS EXIF/XMP methods return placeholders even though core has partial EXIF extraction. | P2 | Choose an explicit still-only contract, or implement frame/metadata preservation end to end. Never advertise preservation while the binding returns empty or synthetic values. |

## Detailed solution design

### 1. Re-establish the binding contract

The binding should be treated as an adapter with its own stable JavaScript
signature, not as a collection of calls that happen to compile against one
core revision.

- `quantize`: define the JavaScript parameter order and defaults, then map to
  the core's `colors`, `kmeans`, palette, dither, and method arguments.
- `reduce`: decide whether the public API accepts one factor or independent
  `xFactor`/`yFactor`; map explicitly rather than relying on a stale one-factor
  call.
- `colorize`: expose the optional midpoint and point controls, or document the
  fixed defaults and pass them explicitly.
- `getColor`: pass alpha to core and convert the `ColorValue` enum into the
  mode-appropriate JavaScript scalar or array. Do not destructure it as a
  four-tuple.

The first acceptance gate should be a clean compile, not a generated-package
smoke test. Run both the default and explicit WASM feature combinations, then
run the package test against freshly generated output.

### 2. Define output as a container API

The app needs a format contract separate from raw pixels:

```text
Image.open(encoded bytes)  -> Image
Image.encode("png", options)  -> container bytes
Image.toBytes()             -> raw mode bytes
Image.toBytesEncoded(...)   -> raw encoder/layout bytes
```

`encode` should accept a normalized format name and a small typed options
object. Unsupported or target-unavailable formats should return a structured
error that the app can turn into a disabled UI state. The first format matrix
should cover PNG, JPEG, GIF, BMP, TIFF, WebP, and ICO with magic-byte checks,
decode-after-encode checks, dimensions, mode, and pixel parity. AVIF belongs in
the matrix only when a WASM encoder has passed the same checks.

The binding should not overload `toBytesEncoded()` to mean container encoding;
its current core implementation intentionally handles raw byte layouts such as
`BGR` and `BGRA`.

### 3. Resolve the dependency and release boundary

The preferred release topology is:

- `fontdone` is a published, exact-version dependency of `pillow-rs`.
- Local developers who have the sibling checkout use a local Cargo patch that
  is not required by a clean clone.
- `image-slash-star` remains pinned to a reviewed revision; any revision change
  is accompanied by the codec feature and WASM capability matrix.
- `pillow-rs-js/package.json` is the npm package manifest and owns the `.` and
  `./extra` exports.
- `pkg/` remains generated and ignored; CI builds it, tests the npm dry-run
  manifest, and publishes from the intended package root.

Before publishing, test from a checkout that contains only this repository:

1. Install the documented Rust toolchain and WASM target.
2. Run Cargo metadata and the WASM check with `--locked`.
3. Build both generated package variants.
4. Run `npm pack --dry-run` and inspect the file list.
5. Install the resulting tarball in a temporary consumer and exercise both
   import paths.

This clean-checkout test is the gate that catches both the relative `fontdone`
path and the wrong npm publish directory.

### 4. Make the browser runtime honest

The app adapter should own runtime policy:

- `ready()` resolves the selected WASM artifact exactly once.
- A Worker receives `{revision, input, operations, format, options}`.
- The main thread publishes only the result whose revision is still current.
- Preview operations use bounded dimensions and resource limits.
- Full-resolution output is only enabled for formats reported as available by
  the loaded artifact.

This gives safe stale-result behavior immediately. A future cancellation API
should be added only after the locked codec revision and `pillow-rs` core expose
the same token contract. It should report whether cancellation is cooperative,
which checkpoints are implemented, and whether an encoder can still spend time
inside an uninterruptible codec call.

### 5. Decide still-image scope before metadata work

For Tiny Image Star's first release, the lowest-risk contract is:

- accept still images only;
- reject multi-frame inputs with a named unsupported-operation error, unless the
  app explicitly chooses a first-frame policy;
- expose actual EXIF bytes only when the selected output path can preserve them;
- do not claim XMP preservation until it is parsed, represented, and re-emitted;
- never return `{}` as a substitute for unavailable metadata.

If animation or metadata preservation is a product requirement, it should be a
separate design lane covering frame storage, seek/tell semantics, output
sequence encoders, metadata ownership, and exact fixture coverage.

## Proposed implementation sequence

### Phase 0 — build restoration

- Align the four JS calls with the current core APIs.
- Add the two WASM compile checks to the maintained CI/release path.
- Regenerate packages only after both checks pass.

### Phase 1 — clean packaging

- Choose and reserve the npm package name.
- Resolve `fontdone` as a clean-checkout dependency.
- Change the release workflow to publish the root package or an explicit
  staging directory.
- Add a source-only checkout test and a tarball consumer test.

### Phase 2 — output contract

- Bind `Image::encode(format)` with normalized options and structured errors.
- Add format capability discovery to the package boundary.
- Add exact output fixtures for each enabled format.
- Keep AVIF disabled in WASM until its encoder is independently proven.

### Phase 3 — interactive fidelity

- Add Worker revision fencing and resource limits in the app adapter.
- Upgrade the pinned codec/core path for cooperative cancellation if required.
- Implement or explicitly reject animation and metadata preservation.

## Acceptance criteria

The proposal is ready to implement when all of these are agreed:

- [ ] `cargo check -p pillow-rs-js --target wasm32-unknown-unknown --locked`
  passes from a clean checkout.
- [ ] The explicit WASM feature lane passes the same check.
- [ ] `npm run build:release` and `npm run test:package` pass on fresh output.
- [ ] `npm pack --dry-run` contains the documented package name, entry points,
  both WASM variants, declarations, license, and required notices.
- [ ] Installing the tarball works without a sibling `fontdone` checkout.
- [ ] Every promised output format passes magic-byte, decode, dimension, mode,
  and pixel checks.
- [ ] Unsupported AVIF behavior is a structured capability/error result, not a
  malformed-input claim.
- [ ] Worker revision tests prove that stale results never replace newer ones.
- [ ] Metadata and animation behavior is either implemented and tested or
  explicitly rejected by the public contract.

## Evidence classification and open decisions

### Proved at review revision

- The current binding does not compile against the current core API.
- Core exposes generic format encoding; the JS binding exposes PNG-only `save`.
- The WASM feature mapping excludes `image-avif`.
- `fontdone` is a relative path dependency and blocks Cargo packaging.
- The npm manifest, documentation, generated manifests, and release target name
  different package boundaries.
- The current core/binding animation and metadata surfaces are incomplete.

### Declared but stale or unverified

- `docs/wasm-core-extra-packaging.md` records a passing packaging matrix, but
  the current WASM compile gate fails before that matrix can be reproduced.
- The same record describes different `core` and `extra` codec surfaces, while
  the current JS feature aliases build the same codec surface for both.
- The README and CONTRIBUTING npm instructions describe `@pillow-rs/wasm`,
  while the current manifest publishes `pillow-rs`.
- Generated `pkg` artifacts are present locally, but they are not source-
  controlled release inputs.

The existing [WASM packaging record](wasm-core-extra-packaging.md) is useful
historical context, but its acceptance claims must be rerun at the current
revision before they are treated as release evidence.

### Unknown and requiring an explicit decision

- Whether `@pillow-rs/wasm` is available for publication under the intended
  owner.
- Whether `fontdone` version `2.14.3-alpha.1` is already published and suitable
  for the release dependency.
- Whether Tiny Image Star requires AVIF, animation, EXIF, or XMP preservation
  in its first release.
- Whether the app will consume npm artifacts or same-repository CI artifacts.

The initial Cargo publish dry-run could not reach crates.io in this environment;
the local `cargo package --offline` failure is sufficient to establish the
relative-path packaging blocker.
