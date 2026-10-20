# Security Policy

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Email **security@tpt-solutions.example** (replace this address with the real security contact before the first public release) with:

- a description of the issue and its impact,
- steps to reproduce, or a proof of concept,
- the affected version or commit.

We aim to acknowledge reports within 5 working days and to agree a disclosure timeline with you. Please give us a reasonable window to fix the issue before public disclosure.

## Scope

In scope:

- Parsing of untrusted GPS input (NMEA and JSON) that could crash or corrupt state.
- Sync authentication, signature verification and replay protection in the backend.
- Handling of device keys and API keys.
- Data exposure beyond the documented privacy model (raw GPS leaving the device without opt-in).

Out of scope:

- Accuracy of distance or RUC estimates. These are reference values and are not legal records. See the NZTA disclaimer in the README.

## Supported versions

Until `v1.0.0`, only the latest tagged release receives security fixes.
