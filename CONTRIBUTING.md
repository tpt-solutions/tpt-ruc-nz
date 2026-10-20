# Contributing to tpt-ruc-nz

Thanks for your interest. This project is maintained by a small team, so the rules below are kept short.

## Licence and sign-off

- All contributions are accepted under the project's dual licence, MIT OR Apache-2.0.
- Contributors certify their work under the [Developer Certificate of Origin](https://developercertificate.org/) (DCO), not a CLA. Add a sign-off line to each commit with `git commit -s`:

  ```
  Signed-off-by: Your Name <you@example.com>
  ```

- Every source file starts with an SPDX header (see [docs/LICENSING.md](docs/LICENSING.md)).
- New dependencies must pass `cargo deny check`. Only permissive licences are allowed.

## Before opening a pull request

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

CI runs the same checks, plus `no_std` builds for `thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf` and `wasm32-unknown-unknown`.

## Code guidelines

- `tpt-ruc-core` must stay `no_std` and must not depend on anything that needs `std` by default.
- The crate forbids `unsafe` code.
- Parsing and maths changes need unit tests. Parser changes should include a real-world sentence where possible.
- Rates and RUC rules must cite an NZTA source in the docs and the commit message. Do not change a rate without a source.

## Commit messages

Write the subject in the imperative, under 72 characters. Explain the why in the body where it is not obvious.

## Scope and disclaimers

This project is a reference implementation and is not an NZTA-approved distance recorder. Changes that remove or weaken the Section 43 disclaimer in the README or UI will not be accepted.
