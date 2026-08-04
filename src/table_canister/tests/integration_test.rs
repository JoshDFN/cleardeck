// Integration tests for table canister
//
// NOTE: True integration tests for Internet Computer canisters require
// either the IC replica or PocketIC framework.
//
// Where the poker logic is tested now:
// - src/poker_core/tests/unit_tests.rs      hand evaluation, deck, shuffle, side pots,
//                                          against the REAL engine rather than a copy
// - src/poker_core/tests/golden_vectors.rs  behaviour lock vs the pre-refactor engine
// - src/table_canister/tests/unit_tests.rs  the seam between this canister and
//                                          poker_core (type identity, side-pot adapter)
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
