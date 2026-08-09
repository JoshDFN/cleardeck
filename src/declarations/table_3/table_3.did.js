export const idlFactory = ({ IDL }) => {
  const Currency = IDL.Variant({ 'BTC' : IDL.Null, 'ICP' : IDL.Null });
  const TableConfig = IDL.Record({
    'small_blind' : IDL.Nat64,
    'time_bank_secs' : IDL.Nat64,
    'action_timeout_secs' : IDL.Nat64,
    'ante' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'max_players' : IDL.Nat8,
    'currency' : Currency,
    'big_blind' : IDL.Nat64,
    'max_buy_in' : IDL.Nat64,
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text });
  const Result = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const Result_DepositAudit = IDL.Variant({
    'Ok' : IDL.Tuple(IDL.Nat64, IDL.Nat64, IDL.Nat64),
    'Err' : IDL.Text,
  });
  const Result_DepositCensus = IDL.Variant({
    'Ok' : IDL.Tuple(
      IDL.Nat64,
      IDL.Vec(IDL.Tuple(IDL.Principal, IDL.Nat64, IDL.Nat64)),
      IDL.Vec(IDL.Principal),
    ),
    'Err' : IDL.Text,
  });
  const Result_5 = IDL.Variant({ 'Ok' : TableConfig, 'Err' : IDL.Text });
  const CommitmentCheckArgs = IDL.Record({
    'seed_hash' : IDL.Text,
    'revealed_seed' : IDL.Text,
  });
  const CommitmentMatch = IDL.Record({
    'committed_hash' : IDL.Text,
    'computed_hash' : IDL.Text,
    'revealed_seed' : IDL.Text,
    'this_does_not_prove' : IDL.Text,
    'this_proves' : IDL.Text,
  });
  const CommitmentNoMatch = IDL.Record({
    'meaning' : IDL.Text,
    'seed_hash_field' : IDL.Text,
    'revealed_seed_field' : IDL.Text,
    'computed_from_seed_hash' : IDL.Text,
    'computed_from_revealed_seed' : IDL.Text,
  });
  const CommitmentMalformed = IDL.Record({
    'field' : IDL.Text,
    'character_length' : IDL.Nat64,
    'reason' : IDL.Text,
  });
  const CommitmentCheck = IDL.Variant({
    'FieldsSwapped' : CommitmentMatch,
    'Match' : CommitmentMatch,
    'NoMatch' : CommitmentNoMatch,
    'Malformed' : CommitmentMalformed,
  });
  const TimeoutCheckResult = IDL.Variant({
    'AutoDealReady' : IDL.Null,
    'PlayerTimedOut' : IDL.Nat8,
    'NoAction' : IDL.Null,
  });
  const ActionTimer = IDL.Record({
    'player_seat' : IDL.Nat8,
    'using_time_bank' : IDL.Bool,
    'expires_at' : IDL.Nat64,
    'started_at' : IDL.Nat64,
  });
  const LedgerIntentView = IDL.Record({
    'id' : IDL.Nat64,
    'who' : IDL.Principal,
    'retry_deadline_ns' : IDL.Nat64,
    'kind' : IDL.Text,
    'attempts' : IDL.Nat32,
    'opened_at_ns' : IDL.Nat64,
    'amount' : IDL.Nat64,
    'leased_until_ns' : IDL.Opt(IDL.Nat64),
  });
  const Rank = IDL.Variant({
    'Ace' : IDL.Null,
    'Six' : IDL.Null,
    'Ten' : IDL.Null,
    'Two' : IDL.Null,
    'Eight' : IDL.Null,
    'Seven' : IDL.Null,
    'Five' : IDL.Null,
    'Four' : IDL.Null,
    'Jack' : IDL.Null,
    'King' : IDL.Null,
    'Nine' : IDL.Null,
    'Three' : IDL.Null,
    'Queen' : IDL.Null,
  });
  const Suit = IDL.Variant({
    'Diamonds' : IDL.Null,
    'Hearts' : IDL.Null,
    'Clubs' : IDL.Null,
    'Spades' : IDL.Null,
  });
  const Card = IDL.Record({ 'rank' : Rank, 'suit' : Suit });
  const SolvencyVerdict = IDL.Variant({
    'CannotPayEveryone' : IDL.Null,
    'CanPayEveryone' : IDL.Null,
    'Unknown' : IDL.Null,
  });
  const CustodyStatus = IDL.Record({
    'total' : IDL.Nat64,
    'chips_at_table' : IDL.Nat64,
    'committed_is_stuck' : IDL.Bool,
    'committed_in_pot' : IDL.Nat64,
    'canister_shortfall_e8s' : IDL.Opt(IDL.Nat64),
    'unswept_deposit_observed_at_ns' : IDL.Opt(IDL.Nat64),
    'advice' : IDL.Text,
    'unswept_deposit' : IDL.Nat64,
    'escrow' : IDL.Nat64,
    'unfinished_ledger_ops' : IDL.Nat64,
    'canister_solvency' : SolvencyVerdict,
    'abandonable_in_ns' : IDL.Opt(IDL.Nat64),
  });
  const CycleStatus = IDL.Record({
    'reserved_for_freezing' : IDL.Nat,
    'next_wake_at' : IDL.Opt(IDL.Nat64),
    'observed_burn_per_day' : IDL.Nat,
    'balance' : IDL.Nat,
    'sample_window_secs' : IDL.Nat64,
    'liquid_balance' : IDL.Nat,
    'measurement_is_meaningful' : IDL.Bool,
    'clock_last_tick_at' : IDL.Nat64,
    'runway_days' : IDL.Opt(IDL.Nat64),
    'clock_watchdog_armed' : IDL.Bool,
    'recent_window_secs' : IDL.Opt(IDL.Nat64),
    'recent_burn_per_day' : IDL.Opt(IDL.Nat),
    'clock_ticks' : IDL.Nat64,
  });
  const DepositAddressCustody = IDL.Record({
    'transfer_fee' : IDL.Nat64,
    'observed_amount' : IDL.Nat64,
    'note' : IDL.Text,
    'observed_at_ns' : IDL.Opt(IDL.Nat64),
    'subaccount' : IDL.Vec(IDL.Nat8),
    'minimum_deposit' : IDL.Nat64,
    'refundable' : IDL.Bool,
    'ledger' : IDL.Principal,
    'canister' : IDL.Principal,
    'address' : IDL.Text,
    'sweepable' : IDL.Bool,
  });
  const FairnessRetention = IDL.Record({
    'table_keeps_last_n_hands' : IDL.Nat64,
    'summary' : IDL.Text,
    'archive_canister' : IDL.Opt(IDL.Principal),
    'table_copy_is_destructible_by_controller' : IDL.Bool,
  });
  const DealtInSeat = IDL.Record({
    'principal' : IDL.Principal,
    'seat' : IDL.Nat8,
  });
  const HandRank = IDL.Variant({
    'StraightFlush' : IDL.Nat8,
    'Straight' : IDL.Nat8,
    'Pair' : IDL.Tuple(IDL.Nat8, IDL.Vec(IDL.Nat8)),
    'FullHouse' : IDL.Tuple(IDL.Nat8, IDL.Nat8),
    'TwoPair' : IDL.Tuple(IDL.Nat8, IDL.Nat8, IDL.Nat8),
    'HighCard' : IDL.Vec(IDL.Nat8),
    'ThreeOfAKind' : IDL.Tuple(IDL.Nat8, IDL.Vec(IDL.Nat8)),
    'Flush' : IDL.Vec(IDL.Nat8),
    'RoyalFlush' : IDL.Null,
    'FourOfAKind' : IDL.Tuple(IDL.Nat8, IDL.Nat8),
  });
  const HistoryPlayerHandRecord = IDL.Record({
    'dealt_in' : IDL.Opt(IDL.Bool),
    'final_hand_rank' : IDL.Opt(HandRank),
    'principal' : IDL.Principal,
    'seat' : IDL.Nat8,
    'hole_cards' : IDL.Opt(IDL.Tuple(Card, Card)),
    'left_mid_hand' : IDL.Opt(IDL.Bool),
    'amount_won' : IDL.Nat64,
    'ending_chips' : IDL.Nat64,
    'starting_chips' : IDL.Nat64,
    'position' : IDL.Text,
    'contributed' : IDL.Opt(IDL.Nat64),
  });
  const ShowdownPlayer = IDL.Record({
    'principal' : IDL.Principal,
    'hand_rank' : IDL.Opt(HandRank),
    'cards' : IDL.Opt(IDL.Tuple(Card, Card)),
    'seat' : IDL.Nat8,
    'amount_won' : IDL.Nat64,
  });
  const PlayerAction = IDL.Variant({
    'Bet' : IDL.Nat64,
    'Call' : IDL.Null,
    'Fold' : IDL.Null,
    'Raise' : IDL.Nat64,
    'AllIn' : IDL.Null,
    'Check' : IDL.Null,
  });
  const ActionRecord = IDL.Record({
    'action' : PlayerAction,
    'seat' : IDL.Nat8,
    'timestamp' : IDL.Nat64,
    'phase' : IDL.Text,
    'amount' : IDL.Nat64,
  });
  const ShuffleProof = IDL.Record({
    'timestamp' : IDL.Nat64,
    'seed_hash' : IDL.Text,
    'revealed_seed' : IDL.Opt(IDL.Text),
  });
  const Winner = IDL.Record({
    'principal' : IDL.Principal,
    'hand_rank' : IDL.Opt(HandRank),
    'cards' : IDL.Opt(IDL.Tuple(Card, Card)),
    'seat' : IDL.Nat8,
    'amount' : IDL.Nat64,
  });
  const HandHistory = IDL.Record({
    'dealt_in' : IDL.Opt(IDL.Vec(DealtInSeat)),
    'participants' : IDL.Opt(IDL.Vec(HistoryPlayerHandRecord)),
    'showdown_players' : IDL.Vec(ShowdownPlayer),
    'hand_number' : IDL.Nat64,
    'actions' : IDL.Vec(ActionRecord),
    'community_cards' : IDL.Vec(Card),
    'shuffle_proof' : ShuffleProof,
    'winners' : IDL.Vec(Winner),
  });
  const HistoryStatus = IDL.Record({
    'last_error' : IDL.Opt(IDL.Text),
    'recorded_ok_since_start' : IDL.Nat64,
    'local_history_cap' : IDL.Nat64,
    'local_history_len' : IDL.Nat64,
    'failed_since_start' : IDL.Nat64,
    'last_recorded_hand' : IDL.Opt(IDL.Nat64),
    'history_canister' : IDL.Opt(IDL.Principal),
    'in_flight' : IDL.Nat64,
    'unrecorded_backlog' : IDL.Nat64,
    'unrecorded_dropped' : IDL.Nat64,
  });
  const SolvencyReport = IDL.Record({
    'pot' : IDL.Nat64,
    'main_observed_at_ns' : IDL.Opt(IDL.Nat64),
    'chips_at_table' : IDL.Nat64,
    'main_account' : IDL.Opt(IDL.Nat64),
    'held' : IDL.Opt(IDL.Nat64),
    'payouts_in_flight' : IDL.Nat64,
    'owed' : IDL.Nat64,
    'shortfall_e8s' : IDL.Opt(IDL.Nat64),
    'pulls_in_flight' : IDL.Nat64,
    'unfinished_incoming' : IDL.Nat64,
    'unswept_deposits' : IDL.Nat64,
    'difference_e8s' : IDL.Opt(IDL.Int),
    'verdict' : SolvencyVerdict,
    'guard_liability' : IDL.Nat64,
    'unattributed_at_main' : IDL.Opt(IDL.Nat64),
    'summary' : IDL.Text,
    'as_of_ns' : IDL.Nat64,
    'main_credited_since_reading' : IDL.Nat64,
    'ledger' : IDL.Principal,
    'committed_stake' : IDL.Nat64,
    'currency' : IDL.Text,
    'deposit_accounts_observed' : IDL.Nat64,
    'sweep_fees_in_flight' : IDL.Nat64,
    'deposit_subaccounts' : IDL.Nat64,
    'deposit_accounts_never_observed' : IDL.Vec(IDL.Principal),
    'main_ledger' : IDL.Opt(IDL.Principal),
    'escrow' : IDL.Nat64,
    'main_debited_since_reading' : IDL.Nat64,
    'deposit_accounts_never_observed_count' : IDL.Nat64,
    'deposit_oldest_observed_at_ns' : IDL.Opt(IDL.Nat64),
  });
  const StuckHandStatus = IDL.Record({
    'refundable_pot' : IDL.Nat64,
    'is_stuck' : IDL.Bool,
    'hand_in_progress' : IDL.Bool,
    'abandonable_in_ns' : IDL.Opt(IDL.Nat64),
  });
  const LastAction = IDL.Variant({
    'Bet' : IDL.Record({ 'amount' : IDL.Nat64 }),
    'PostBlind' : IDL.Record({ 'amount' : IDL.Nat64 }),
    'Call' : IDL.Record({ 'amount' : IDL.Nat64 }),
    'Fold' : IDL.Null,
    'Raise' : IDL.Record({ 'amount' : IDL.Nat64 }),
    'AllIn' : IDL.Record({ 'amount' : IDL.Nat64 }),
    'Check' : IDL.Null,
  });
  const LastActionInfo = IDL.Record({
    'action' : LastAction,
    'seat' : IDL.Nat8,
    'timestamp' : IDL.Nat64,
  });
  const PlayerStatus = IDL.Variant({
    'SittingOut' : IDL.Null,
    'Active' : IDL.Null,
    'Disconnected' : IDL.Null,
  });
  const Player = IDL.Record({
    'status' : PlayerStatus,
    'timeout_count' : IDL.Nat8,
    'principal' : IDL.Principal,
    'is_sitting_out_next_hand' : IDL.Bool,
    'has_folded' : IDL.Bool,
    'sitting_out_since' : IDL.Opt(IDL.Nat64),
    'chips' : IDL.Nat64,
    'seat' : IDL.Nat8,
    'hole_cards' : IDL.Opt(IDL.Tuple(Card, Card)),
    'total_bet_this_hand' : IDL.Nat64,
    'current_bet' : IDL.Nat64,
    'has_acted_this_round' : IDL.Bool,
    'time_bank_remaining' : IDL.Nat64,
    'broke_at' : IDL.Opt(IDL.Nat64),
    'last_seen' : IDL.Nat64,
    'is_all_in' : IDL.Bool,
  });
  const GamePhase = IDL.Variant({
    'Flop' : IDL.Null,
    'Turn' : IDL.Null,
    'River' : IDL.Null,
    'Showdown' : IDL.Null,
    'HandComplete' : IDL.Null,
    'WaitingForPlayers' : IDL.Null,
    'PreFlop' : IDL.Null,
  });
  const SidePot = IDL.Record({
    'eligible_players' : IDL.Vec(IDL.Nat8),
    'amount' : IDL.Nat64,
  });
  const DepartedStake = IDL.Record({
    'principal' : IDL.Principal,
    'hand_number' : IDL.Nat64,
    'seat' : IDL.Nat8,
    'contributed' : IDL.Nat64,
  });
  const TableState = IDL.Record({
    'id' : IDL.Nat64,
    'pot' : IDL.Nat64,
    'min_raise' : IDL.Nat64,
    'bb_has_option' : IDL.Bool,
    'action_on' : IDL.Nat8,
    'action_timer' : IDL.Opt(ActionTimer),
    'dealer_seat' : IDL.Nat8,
    'hand_number' : IDL.Nat64,
    'deck' : IDL.Vec(Card),
    'big_blind_seat' : IDL.Nat8,
    'auto_deal_at' : IDL.Opt(IDL.Nat64),
    'last_action' : IDL.Opt(LastActionInfo),
    'players' : IDL.Vec(IDL.Opt(Player)),
    'current_bet' : IDL.Nat64,
    'last_aggressor' : IDL.Opt(IDL.Nat8),
    'deck_index' : IDL.Nat64,
    'community_cards' : IDL.Vec(Card),
    'small_blind_seat' : IDL.Nat8,
    'first_hand' : IDL.Bool,
    'phase' : GamePhase,
    'config' : TableConfig,
    'side_pots' : IDL.Vec(SidePot),
    'shuffle_proof' : IDL.Opt(ShuffleProof),
    'departed_stakes' : IDL.Opt(IDL.Vec(DepartedStake)),
  });
  const Result_2 = IDL.Variant({ 'Ok' : TableState, 'Err' : IDL.Text });
  const PlayerView = IDL.Record({
    'status' : PlayerStatus,
    'is_self' : IDL.Bool,
    'principal' : IDL.Principal,
    'has_folded' : IDL.Bool,
    'chips' : IDL.Nat64,
    'seat' : IDL.Nat8,
    'hole_cards' : IDL.Opt(IDL.Tuple(Card, Card)),
    'display_name' : IDL.Opt(IDL.Text),
    'current_bet' : IDL.Nat64,
    'is_all_in' : IDL.Bool,
  });
  const TableView = IDL.Record({
    'id' : IDL.Nat64,
    'pot' : IDL.Nat64,
    'min_raise' : IDL.Nat64,
    'time_bank_remaining_secs' : IDL.Opt(IDL.Nat64),
    'action_on' : IDL.Nat8,
    'call_amount' : IDL.Nat64,
    'my_seat' : IDL.Opt(IDL.Nat8),
    'hand_is_unmovable' : IDL.Bool,
    'dealer_seat' : IDL.Nat8,
    'hand_number' : IDL.Nat64,
    'min_bet' : IDL.Nat64,
    'big_blind_seat' : IDL.Nat8,
    'can_check' : IDL.Bool,
    'last_action' : IDL.Opt(LastActionInfo),
    'last_hand_winners' : IDL.Vec(Winner),
    'time_remaining_secs' : IDL.Opt(IDL.Nat64),
    'players' : IDL.Vec(IDL.Opt(PlayerView)),
    'current_bet' : IDL.Nat64,
    'community_cards' : IDL.Vec(Card),
    'small_blind_seat' : IDL.Nat8,
    'using_time_bank' : IDL.Bool,
    'my_committed_in_pot' : IDL.Nat64,
    'phase' : GamePhase,
    'config' : TableConfig,
    'side_pots' : IDL.Vec(SidePot),
    'shuffle_proof' : IDL.Opt(ShuffleProof),
    'is_my_turn' : IDL.Bool,
    'can_raise' : IDL.Bool,
  });
  const Result_DepositCustody = IDL.Variant({
    'Ok' : DepositAddressCustody,
    'Err' : IDL.Text,
  });
  const MainAccountObservation = IDL.Record({
    'debited_since' : IDL.Nat64,
    'observed_at_ns' : IDL.Nat64,
    'credited_since' : IDL.Nat64,
    'ledger' : IDL.Principal,
    'amount' : IDL.Nat64,
  });
  const Result_MainCustody = IDL.Variant({
    'Ok' : MainAccountObservation,
    'Err' : IDL.Text,
  });
  const Result_Solvency = IDL.Variant({
    'Ok' : SolvencyReport,
    'Err' : IDL.Text,
  });
  const Result_IntentLine = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text });
  const Result_IntentReport = IDL.Variant({
    'Ok' : IDL.Vec(IDL.Text),
    'Err' : IDL.Text,
  });
  const Result_3 = IDL.Variant({
    'Ok' : IDL.Tuple(Card, Card),
    'Err' : IDL.Text,
  });
  const Result_4 = IDL.Variant({ 'Ok' : ShuffleProof, 'Err' : IDL.Text });
  const UtxoOutpoint = IDL.Record({
    'txid' : IDL.Vec(IDL.Nat8),
    'vout' : IDL.Nat32,
  });
  const Utxo = IDL.Record({
    'height' : IDL.Nat32,
    'value' : IDL.Nat64,
    'outpoint' : UtxoOutpoint,
  });
  const UtxoStatus = IDL.Variant({
    'ValueTooSmall' : Utxo,
    'Tainted' : Utxo,
    'Minted' : IDL.Record({
      'minted_amount' : IDL.Nat64,
      'block_index' : IDL.Nat64,
      'utxo' : Utxo,
    }),
    'Checked' : Utxo,
  });
  return IDL.Service({
    'abandon_stuck_hand' : IDL.Func([], [Result_1], []),
    'add_controller' : IDL.Func([IDL.Principal], [Result], []),
    'admin_audit_deposit_custody' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_DepositAudit],
        [],
      ),
    'admin_get_all_balances' : IDL.Func(
        [],
        [
          IDL.Variant({
            'Ok' : IDL.Tuple(
              IDL.Nat64,
              IDL.Vec(IDL.Tuple(IDL.Principal, IDL.Nat64)),
            ),
            'Err' : IDL.Text,
          }),
        ],
        ['query'],
      ),
    'admin_get_balance' : IDL.Func([IDL.Principal], [Result_1], ['query']),
    'admin_get_deposit_custody' : IDL.Func(
        [],
        [Result_DepositCensus],
        ['query'],
      ),
    'admin_get_table_chips' : IDL.Func([], [Result_1], ['query']),
    'admin_reinit_table' : IDL.Func([TableConfig], [Result], []),
    'admin_return_all_chips_to_escrow' : IDL.Func([], [Result_1], []),
    'admin_update_config' : IDL.Func([TableConfig], [Result_5], []),
    'buy_in' : IDL.Func([IDL.Nat8, IDL.Nat64], [Result], []),
    'cash_out' : IDL.Func([], [Result_1], []),
    'check_shuffle_commitment' : IDL.Func(
        [CommitmentCheckArgs],
        [CommitmentCheck],
        ['query'],
      ),
    'check_timeouts' : IDL.Func([], [TimeoutCheckResult], []),
    'claim_external_deposit' : IDL.Func([], [Result_1], []),
    'deposit' : IDL.Func([IDL.Nat64], [Result_1], []),
    'dev_faucet' : IDL.Func([IDL.Nat64], [Result_1], []),
    'did_player_show' : IDL.Func([IDL.Nat8], [IDL.Bool], ['query']),
    'flush_unrecorded_hands' : IDL.Func([], [Result_1], []),
    'get_action_timer' : IDL.Func([], [IDL.Opt(ActionTimer)], ['query']),
    'get_all_ledger_intents' : IDL.Func(
        [],
        [IDL.Vec(LedgerIntentView)],
        ['query'],
      ),
    'get_balance' : IDL.Func([], [IDL.Nat64], ['query']),
    'get_btc_deposit_address' : IDL.Func(
        [],
        [IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text })],
        [],
      ),
    'get_community_cards' : IDL.Func([], [IDL.Vec(Card)], ['query']),
    'get_controllers' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'get_custody_status' : IDL.Func([], [CustodyStatus], ['query']),
    'get_cycle_status' : IDL.Func([], [CycleStatus], ['query']),
    'get_deposit_address' : IDL.Func([], [IDL.Text], ['query']),
    'get_deposit_custody' : IDL.Func([], [DepositAddressCustody], ['query']),
    'get_deposit_replay_state' : IDL.Func(
        [],
        [IDL.Nat64, IDL.Nat64],
        ['query'],
      ),
    'get_deposit_subaccount' : IDL.Func([], [IDL.Vec(IDL.Nat8)], ['query']),
    'get_display_name' : IDL.Func(
        [IDL.Principal],
        [IDL.Opt(IDL.Text)],
        ['query'],
      ),
    'get_fairness_retention' : IDL.Func([], [FairnessRetention], ['query']),
    'get_hand_history' : IDL.Func(
        [IDL.Nat64],
        [IDL.Opt(HandHistory)],
        ['query'],
      ),
    'get_history_canister' : IDL.Func([], [IDL.Opt(IDL.Principal)], ['query']),
    'get_history_status' : IDL.Func([], [HistoryStatus], ['query']),
    'get_max_players' : IDL.Func([], [IDL.Nat8], ['query']),
    'get_my_cards' : IDL.Func([], [IDL.Opt(IDL.Tuple(Card, Card))], ['query']),
    'get_my_ledger_intents' : IDL.Func(
        [],
        [IDL.Vec(LedgerIntentView)],
        ['query'],
      ),
    'get_player_count' : IDL.Func([], [IDL.Nat8], ['query']),
    'get_pot' : IDL.Func([], [IDL.Nat64], ['query']),
    'get_shown_cards' : IDL.Func(
        [IDL.Nat8],
        [IDL.Opt(IDL.Tuple(Card, Card))],
        ['query'],
      ),
    'get_shuffle_proof' : IDL.Func([], [IDL.Opt(ShuffleProof)], ['query']),
    'get_solvency' : IDL.Func([], [SolvencyReport], ['query']),
    'get_stuck_hand_status' : IDL.Func([], [StuckHandStatus], ['query']),
    'get_table_state' : IDL.Func([], [Result_2], ['query']),
    'get_table_view' : IDL.Func([], [IDL.Opt(TableView)], ['query']),
    'get_time_remaining' : IDL.Func([], [IDL.Opt(IDL.Nat64)], ['query']),
    'heartbeat' : IDL.Func([], [Result], []),
    'is_dev_mode' : IDL.Func([], [IDL.Bool], ['query']),
    'join_table' : IDL.Func([IDL.Nat8], [Result], []),
    'leave_table' : IDL.Func([], [Result_1], []),
    'notify_deposit' : IDL.Func([IDL.Nat64], [Result_1], []),
    'player_action' : IDL.Func([PlayerAction], [Result], []),
    'refresh_deposit_custody' : IDL.Func([], [Result_DepositCustody], []),
    'refresh_main_account_custody' : IDL.Func([], [Result_MainCustody], []),
    'refresh_solvency' : IDL.Func([], [Result_Solvency], []),
    'refund_external_deposit' : IDL.Func([], [Result_1], []),
    'reload' : IDL.Func([IDL.Nat64], [Result_1], []),
    'remove_controller' : IDL.Func([IDL.Principal], [Result], []),
    'reset_table' : IDL.Func([TableConfig], [Result], []),
    'resolve_ledger_intent' : IDL.Func([IDL.Nat64], [Result_IntentLine], []),
    'resolve_my_ledger_intents' : IDL.Func([], [Result_IntentReport], []),
    'set_dev_mode' : IDL.Func([IDL.Bool], [Result], []),
    'set_display_name' : IDL.Func([IDL.Opt(IDL.Text)], [Result], []),
    'set_history_canister' : IDL.Func([IDL.Opt(IDL.Principal)], [Result], []),
    'show_cards' : IDL.Func([], [Result_3], []),
    'sit_in' : IDL.Func([], [Result], []),
    'sit_out' : IDL.Func([], [Result], []),
    'sit_out_next_hand' : IDL.Func([], [Result], []),
    'start_new_hand' : IDL.Func([], [Result_4], []),
    'update_btc_balance' : IDL.Func(
        [],
        [IDL.Variant({ 'Ok' : IDL.Vec(UtxoStatus), 'Err' : IDL.Text })],
        [],
      ),
    'use_time_bank' : IDL.Func([], [Result_1], []),
    'verify_shuffle' : IDL.Func([IDL.Text, IDL.Text], [IDL.Bool], ['query']),
    'withdraw' : IDL.Func([IDL.Nat64], [Result_1], []),
  });
};
export const init = ({ IDL }) => {
  const Currency = IDL.Variant({ 'BTC' : IDL.Null, 'ICP' : IDL.Null });
  const TableConfig = IDL.Record({
    'small_blind' : IDL.Nat64,
    'time_bank_secs' : IDL.Nat64,
    'action_timeout_secs' : IDL.Nat64,
    'ante' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'max_players' : IDL.Nat8,
    'currency' : Currency,
    'big_blind' : IDL.Nat64,
    'max_buy_in' : IDL.Nat64,
  });
  return [TableConfig];
};
