# Release Checklist

Use this checklist for future `0.2.x` and `0.3.x` releases.

## Before Tagging

- Confirm the milestone name and version in `Cargo.toml`, `README.md`, `CHANGELOG.md`, roadmap docs, and release notes.
- Keep compatibility language honest and backed by clean-room evidence.
- Confirm no Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied server/modding project code is present.
- Update `docs/COMPATIBILITY.md` and `docs/VANILLA_PARITY_MATRIX.md`.
- Update `docs/releases/<version>.md`.
- Check that `.github/FUNDING.yml` still reflects real funding accounts.

Run:

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world
```

For compatibility releases, also run the manual real-client acceptance pass from [COMPATIBILITY.md](COMPATIBILITY.md).

## Tagging

For `v0.2.0`:

```bash
git tag -a v0.2.0 -m "Aurelia 0.2.0 - Vanilla Parity Foundation"
git push origin v0.2.0
```

For later releases, use the same pattern:

```bash
git tag -a v0.2.1 -m "Aurelia 0.2.1"
git push origin v0.2.1
```

## GitHub Release

- Create a GitHub release from the tag.
- Use the matching file in `docs/releases/` as the release-note draft.
- Mark prerelease status if compatibility is still experimental.
- Do not upload Minecraft jars, assets, generated sources, or decompiled source.
- Link the compatibility and clean-room docs.

## After Release

- Move shipped `CHANGELOG.md` entries out of `Unreleased`.
- Open or update the next milestone.
- Re-check repository topics, description, website, CI, and sponsorship visibility.
