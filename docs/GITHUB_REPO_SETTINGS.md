# GitHub Repository Settings

These values are recommended for manually polishing the public GitHub repository page.

## Metadata

Recommended description:

```text
Clean-room Minecraft Beta 1.7.3 server rewrite in Rust with a future region-threaded architecture.
```

Recommended website:

```text
luxorium.dev
```

Recommended topics:

- `minecraft`
- `minecraft-server`
- `beta-173`
- `rust`
- `clean-room`
- `protocol`
- `game-server`
- `region-threaded`
- `legacy-minecraft`
- `open-source`

## Features

- Issues: enabled.
- Pull requests: enabled.
- Discussions: optional, recommended once the repo has active users.
- Sponsorships: enabled after GitHub Sponsors is verified.
- Releases: publish a first `v0.2.0` release once docs and CI are merged.

## Branch Protection

Recommended once CI is active:

- Require pull requests before merging.
- Require the `CI / Rust` check to pass.
- Require branches to be up to date before merging if the repo becomes busy.
- Require conversation resolution before merging.

## Helper Script

The metadata helper prints the recommended `gh repo edit` commands by default:

```bash
scripts/setup-github-metadata.sh
```

Apply changes only when ready:

```bash
scripts/setup-github-metadata.sh --apply
```

The script does not make destructive changes and explains what to do if `gh` is missing.
