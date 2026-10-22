# Maintenance plan

Written for: the maintainer of this project. It covers what must be kept up to date after v1, and who does each job. This project hosts nothing, so every task below is about the code, the rate data and the operators' copies.

## Dependency updates

- **Tool:** Dependabot, configured in `.github/dependabot.yml`, for Cargo, npm and GitHub Actions.
- **Routine:** review Dependabot pull requests weekly. Patch and minor updates merge once CI is green. Major updates get a short note in the pull request about what changed.
- **Licence:** every new dependency must pass the licence policy in `deny.toml` (for Rust) and `docs/LICENSING.md` (for the PWA). `cargo deny` and the PWA licence check run in CI, so a bad licence fails the build.
- **Security:** `cargo audit` and `cargo deny` advisories are checked in CI. A RustSec advisory on a shipped dependency is fixed before the next release, even if it is only a patch.

## NZTA rate changes

- **Source:** NZTA's published rates. The procedure is in `docs/nzta/rate-update-procedure.md`.
- **Routine:** check the NZTA page each quarter, and whenever a change is announced. Record the check date in `docs/nzta/rates-research.md`, even when nothing has changed.
- **Rule:** a new rate table is added beside the old one, with its effective date. Older tables are never edited.

## Issue triage

- **Where:** GitHub issues on `tpt-solutions/tpt-ruc-nz`.
- **Routine:** look at new issues weekly. Label each one as a bug, a question, a rate change, or a feature request. Answer questions that the docs already cover by pointing to the doc.
- **Security reports:** follow `SECURITY.md`. Do not discuss them in a public issue.
- **Scope:** features that need hosting, a hosted service, or Section 43 approval are declined, because the project does not host anything and does not apply for approval (`todo.md`, decisions locked in).

## Operator copies

Operators run their own backend and PWA. The project cannot push fixes to them, so:

- **Changelog:** every release lists its security fixes first, in `CHANGELOG.md`.
- **Upgrade notes:** a release that changes the database, the API or the signing format says so at the top of its changelog entry.
- **Backups:** `docs/guides/reverse-proxy-and-backups.md` covers backups. Nothing in this plan changes that.

## Release cadence

- **Routine:** a release when a security fix or a rate change needs shipping, and otherwise about twice a year. Releases follow `docs/RELEASING.md`.
- **Gate:** no release until the first real-device drive test is done (`todo.md`, decided 2026-10-22).

## Who does what

The project is maintained by the owner. Nothing here needs a second person, but a security report should be read by someone other than the person who wrote the affected code, if one is available.
