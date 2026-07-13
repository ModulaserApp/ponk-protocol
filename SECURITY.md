# Security policy

## Supported versions

Until the first tagged release, security fixes are applied to the `main` branch. After releases begin, the latest released minor version will receive security fixes.

## Report a vulnerability privately

Do not open a public issue for a suspected vulnerability.

Use GitHub's **Security → Report a vulnerability** form for this repository. If private vulnerability reporting is unavailable, email `info@modulaser.app` with the subject `ponk-protocol security report`.

Include:

- the affected revision or version;
- the datagram sequence or minimal reproducer;
- the observed impact, including memory, CPU, panic, or wire-compatibility effects;
- any suggested mitigation;
- whether and when you plan to disclose the issue.

Do not include live credentials, private network captures, or personal data. We will acknowledge a complete report within seven days and coordinate remediation and disclosure with the reporter.

## Security scope

Relevant reports include parser panics, unbounded allocation or CPU use, assembly-limit bypasses, checksum or frame-identity confusion, and unsafe handling of non-finite or out-of-range coordinates.

This crate does not provide laser hardware safety. Missing arming, blanking, motion, energy, projection-zone, DAC, or fail-dark controls in an application are outside this crate's implementation scope, but documentation defects that could cause unsafe integration are welcome as private reports.
