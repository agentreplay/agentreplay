# Contract Tests

These tests validate serialized API payloads against JSON Schemas.

## Run

From the repository root:

```zsh
cargo test -p agentreplay-server contract_outputs
```

Schemas live in `agentreplay-server/schemas/` and should be updated alongside DTO changes.
