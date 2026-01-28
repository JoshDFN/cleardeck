// Integration tests for table canister
//
// NOTE: True integration tests for Internet Computer canisters require
// either the IC replica or PocketIC framework. The unit tests in unit_tests.rs
// cover the core poker logic (hand evaluation, deck operations, side pots).
//
// For full canister integration testing, use:
// - dfx start --background && dfx deploy
// - Run manual tests via dfx canister call commands
// - Or use the PocketIC testing framework: https://github.com/dfinity/ic/tree/master/packages/pocket-ic
//
// Example dfx test commands:
// dfx canister call table_headsup get_table_view '()'
// dfx canister call table_headsup buy_in '(0 : nat8, 500 : nat64)'
// dfx canister call table_headsup start_new_hand '()'

// The actual unit tests are in unit_tests.rs
