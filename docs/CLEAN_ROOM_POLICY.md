# Clean-Room Policy

Aurelia is a clean-room, original Minecraft Beta 1.7.3-compatible server rewrite. The project can study behavior, but implementation code and committed documentation must stay legally clean.

## Allowed References

The following are acceptable when used carefully:

- Public protocol or format documentation.
- Black-box packet traces captured from clean local client/server sessions.
- Original observations from using a clean Beta 1.7.3 client.
- Independently written behavior notes.
- Tests written from public documentation or observed behavior.
- Public issue comments or reports that describe behavior in the author's own words.

Allowed references should be converted into concise notes, tests, or original implementation code. Cite the kind of evidence where useful.

## Forbidden Material

Do not commit, paste, attach, generate, or vendor:

- Mojang source code.
- Minecraft assets.
- Minecraft client or server jars.
- Generated jars or generated sources.
- Decompiled Minecraft source.
- Copied protocol code.
- Copied code from Bukkit, Paper, Folia, Fabric, Forge, or other server/modding projects.
- Proprietary session tokens, credentials, or private user data.

This applies to code, docs, tests, fixtures, issue comments, pull requests, release artifacts, and screenshots when they expose proprietary material.

## Reference Workflow

If you inspect external behavior:

1. Keep reference workspaces outside this repository.
2. Capture behavior as packet traces, screenshots, or written observations where legally safe.
3. Translate observations into original notes.
4. Add tests that assert the behavior.
5. Implement original code that satisfies those tests.

Do not translate decompiled code line by line. Do not preserve copied structure, names, comments, constants, or algorithms from forbidden sources.

## Compatibility Claims

Aurelia should only claim compatibility for behavior that has clean evidence. Acceptable evidence includes:

- Passing tests based on public docs or black-box observations.
- Packet traces with direction, packet ID, payload length, and payload bytes.
- Manual real-client observations described in original words.
- Reproducible issue reports.

When evidence is incomplete, use cautious wording such as "experimental", "provisional", "observed", or "unverified".

## Reviewer Checklist

- No forbidden material is present.
- New compatibility claims cite clean evidence or are worded as provisional.
- New tests do not include proprietary assets or copied source.
- New docs do not instruct contributors to commit jars, assets, or decompiled source.
- Release artifacts do not include Minecraft files.
