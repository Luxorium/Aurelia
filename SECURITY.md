# Security Policy

## Supported Versions

Aurelia is pre-1.0 software. Security fixes are expected to target the `main` branch and the active `0.2.x` development line unless a release branch is created later.

## Reporting A Vulnerability

Use GitHub private vulnerability reporting if it is enabled for this repository. If it is not enabled, open a minimal public issue asking for a private maintainer contact and avoid posting exploit details publicly.

Please include:

- Affected commit or release.
- Reproduction steps.
- Expected and actual impact.
- Relevant logs or traces with secrets redacted.
- Whether the issue requires a real Beta 1.7.3 client, a crafted TCP client, or local filesystem access.

## Clean-Room And Legal Safety

Do not attach Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied server/modding project code to security reports. Describe behavior and evidence in your own words.

## Scope

Useful reports include crashes from malformed input, denial-of-service behavior, unsafe file handling, dependency vulnerabilities, and issues that could expose host or player data. Gameplay parity bugs should usually use the normal issue templates unless they also create a security impact.
