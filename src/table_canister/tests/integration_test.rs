// Integration tests for table canister
//
// NOTE: True integration tests for Internet Computer canisters require
// either the IC replica or PocketIC framework. The unit tests in unit_tests.rs
// cover the core poker logic (hand evaluation, deck operations, side pots).
//
// For full canister integration testing, use:
// - icp network start -d && icp deploy
// - Run manual tests via icp canister call commands
// - Or use the PocketIC testing framework: https://github.com/dfinity/ic/tree/master/packages/pocket-ic
//
// Example icp test commands:
// icp canister call table_headsup get_table_view '()'
// icp canister call table_headsup buy_in '(0 : nat8, 500 : nat64)'
// icp canister call table_headsup start_new_hand '()'

// The actual unit tests are in unit_tests.rs
