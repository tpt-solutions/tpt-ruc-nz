# Versioning and release strategy

## Decision

Releases use **release-please** for version bumps and changelog generation, with **semantic versioning** on tagged releases (`v0.1.0`, `v0.2.0`, ...). Each phase exit tags a release, as set out in `todo.md`.

Why release-please over cargo-dist: the workspace has Rust crates, a PWA and firmware images. release-please manages a single version across the monorepo, writes the changelog from Conventional Commit messages, and opens a release PR. cargo-dist is still used later for binaries, as a second step (Phase 6).

## Conventions

- Use Conventional Commit prefixes (`feat:`, `fix:`, `docs:`, `chore:`). Breaking changes use `feat!:` or a `BREAKING CHANGE:` footer.
- Pre-1.0 (`0.x`): breaking changes bump the minor version. `v1.0.0` is reached only after the Phase 6 licence audit and the NZTA rate-table procedure are in place.
- The RUC rate table has its own effective-dated version. A rate change ships as a data release and is not hidden inside a code change (see the rate-table update procedure in Phase 6).

## Setup status

The release-please workflow is not yet added. It is planned with the first Phase 1 tag, `v0.1.0`.
