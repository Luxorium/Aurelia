## Summary

Describe the change and why it is needed.

## Type

- [ ] Documentation
- [ ] Tests
- [ ] Protocol
- [ ] World/persistence
- [ ] Inventory/gameplay
- [ ] Server/runtime
- [ ] Repository maintenance

## Clean-Room Checklist

- [ ] This PR does not include Mojang source code, Minecraft assets, generated jars, decompiled source, or copied server/modding project code.
- [ ] Compatibility claims are backed by public documentation, black-box traces, original observations, independently written notes, or tests.
- [ ] Any external references are summarized in original words.

## Testing

Commands run:

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world
```

## Compatibility Notes

If this affects real-client behavior, include the client version, server command, trace summary, and known limitations.
