# Testing Guide

## Unit Tests

Run unit tests for Rust canisters:
```bash
cargo test --workspace
```

## Integration Tests

Integration tests require a local IC replica:
```bash
# Start local replica
icp network start

# Run integration tests
cargo test --package table_canister --test integration_test
```

## E2E Tests

End-to-end tests require the full stack:
```bash
# Deploy to the local environment
icp deploy -e local

# Run E2E tests (when implemented)
npm test
```

## Test Coverage

Current test coverage:
- ⚠️ Unit tests: Not yet implemented
- ⚠️ Integration tests: Placeholder structure only
- ⚠️ E2E tests: Not yet implemented

## CI

`.github/workflows/ci.yml` runs `cargo build` (wasm32), `cargo test --workspace`,
a candid-interface drift check, and the frontend build on every PR/push.
`.github/workflows/security.yml` runs `cargo audit` + `npm audit`.

## TODO

- [ ] Add unit tests for game logic
- [ ] Add integration tests for canister interactions
- [ ] Add E2E tests for critical user flows
- [ ] Set up test coverage reporting
