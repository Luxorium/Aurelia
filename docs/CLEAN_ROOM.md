# Clean-Room Rules

Aurelia must be an original implementation.

## Hard Rules

- Aurelia must not contain Mojang source code.
- Aurelia must not contain Minecraft assets.
- Aurelia must not generate or distribute a Minecraft jar.
- Aurelia must not copy decompiled Minecraft source.
- Aurelia must not copy Paper, Folia, Bukkit, Fabric, or Forge code.

## Reference Workflow

External tools are allowed only as separate local reference workflows. They may
be used to study Beta 1.7.3 behavior, inspect a local reference workspace, and
compare observations against the original game.

Reference workspaces must stay outside Aurelia source control.

## Contributor Expectations

Contributors who inspect decompiled source should document behavior instead of
copying code. Prefer notes like:

- Packet field order observed from client/server traffic.
- Gameplay behavior reproduced from black-box testing.
- World or entity behavior described in plain English.
- Edge cases captured as tests written with original implementation code.

Implementation should be original and independently structured.

## Review Checklist

- No decompiled source pasted into commits.
- No assets or generated jars committed.
- No copied implementations from server projects.
- Compatibility notes cite observations, tests, or external documentation rather
  than copied source.
