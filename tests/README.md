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
dfx start --background

# Run integration tests
cargo test --package table_canister --test integration_test
```

## E2E Tests

End-to-end tests require the full stack:
```bash
# Deploy to local replica
dfx deploy

# Run E2E tests (when implemented)
npm test
```

## Test Coverage

Current test coverage:
- ⚠️ Unit tests: Not yet implemented
- ⚠️ Integration tests: Placeholder structure only
- ⚠️ E2E tests: Not yet implemented

## TODO

- [ ] Add unit tests for game logic
- [ ] Add integration tests for canister interactions
- [ ] Add E2E tests for critical user flows
- [ ] Set up test coverage reporting
- [ ] Add CI/CD pipeline with tests
