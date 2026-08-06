// Per-METHOD field check: for each canister result the shots harness binds to a
// variable, list the fields the harness reads off it and ask the LIVE idlFactory
// whether that method's return type actually has them. This is the E-61 class:
// a field that exists on a DIFFERENT type.
import { IDL } from '@dfinity/candid';
import { idlFactory as historyIdl } from '/Users/josh/Desktop/cleardeck/src/declarations/history/history.did.js';
import { idlFactory as tableIdl } from '/Users/josh/Desktop/cleardeck/src/declarations/table_1/table_1.did.js';
import { idlFactory as lobbyIdl } from '/Users/josh/Desktop/cleardeck/src/declarations/lobby/lobby.did.js';

const svc = (f) => Object.fromEntries(f({ IDL })._fields);
const H = svc(historyIdl), T = svc(tableIdl), L = svc(lobbyIdl);

// unwrap opt / vec / record down to the record's field names
function fieldsOf(t) {
  let x = t;
  for (let i = 0; i < 8; i += 1) {
    if (x?._fields) return x._fields.map(([n]) => n);
    if (x?._type) { x = x._type; continue; }
    if (Array.isArray(x?._types) && x._types.length === 1) { x = x._types[0]; continue; }
    break;
  }
  return null;
}

// What the harness reads off each call's result, transcribed from
// tools/shots/lib/chain-agreement.mjs.
const reads = [
  ['history.get_hands_by_table -> HandSummary', fieldsOf(H.get_hands_by_table.retTypes[0]),
   ['hand_id', 'hand_number', 'total_pot', 'winners']],
  ['history.get_hand -> HandHistoryRecord', fieldsOf(H.get_hand.retTypes[0]),
   ['total_pot', 'rake', 'winners']],
  ['table.get_hand_history -> HandHistory', fieldsOf(T.get_hand_history.retTypes[0]),
   ['hand_number', 'winners']],
  ['table.get_table_view -> TableView', fieldsOf(T.get_table_view.retTypes[0]),
   ['pot', 'side_pots', 'players', 'phase', 'community_cards', 'current_bet', 'hand_number',
    'last_hand_winners', 'dealer_seat', 'config']],
  ['lobby.get_tables -> TableInfo', fieldsOf(L.get_tables.retTypes[0]),
   ['player_count', 'canister_id', 'name', 'config', 'status']],
];

let bad = 0;
for (const [label, have, want] of reads) {
  if (!have) { console.log(`?? ${label}: could not resolve fields`); continue; }
  const missing = want.filter((f) => !have.includes(f));
  console.log(`\n${label}`);
  console.log(`   declared: ${have.join(', ')}`);
  if (missing.length) { console.log(`   *** HARNESS READS FIELDS THAT DO NOT EXIST: ${missing.join(', ')}`); bad += missing.length; }
  else console.log('   every field the harness reads exists on this type');
}
console.log(`\n${bad} bad read(s)`);
