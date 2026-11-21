# Security Policy

## Supported Versions

Security updates are provided for the following versions of the karis-ky escrow contracts:

| Version | Supported          |
| ------- | ------------------ |
| 7.x     | :white_check_mark: |
| < 7.0   | :x:                |

Only the latest deployed schema version (see `SCHEMA_VERSION` in `escrow/src/lib.rs`)
receives security fixes. Older versions should be upgraded or redeployed.

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues,
discussions, or pull requests.**

Instead, report them privately using GitHub's private vulnerability reporting:

1. Go to the **Security** tab of this repository.
2. Click **Report a vulnerability** (GitHub Security Advisories).

This opens a private advisory visible only to you and the maintainers. If you
are unable to use GitHub Security Advisories, contact the maintainers directly
through the private channel listed in the repository profile.

Please include as much detail as possible:

- A description of the vulnerability and its impact.
- Steps to reproduce or a proof-of-concept.
- Affected versions, contract entrypoints, or components.
- Any suggested remediation.

## Response Timeline

- **Acknowledgment:** within 3 business days of your report.
- **Initial assessment:** within 7 business days, including severity and a
  remediation plan.
- **Fix and disclosure:** we aim to release a fix within 30 days for confirmed
  vulnerabilities, coordinating public disclosure with you once a fix is
  available.

We will keep you informed of progress throughout the process and credit
reporters in the advisory unless you prefer to remain anonymous.
