// THE PLAYER-PROTECTION NOTICES, ONCE.
//
// Five phrases are the product's hard rule: "Unaudited code with known bugs",
// "your funds are NOT safe", "illegal in many jurisdictions", "18+ only" and
// "No rake is taken from any pot on any table". `scripts/dev.sh hygiene` greps
// the source for them and `tools/shots/lib/protected-notices.mjs` hit-tests
// them on the rendered pixels of every scene. They used to live in six files,
// each hand-synced; this module is the one place their words are written.
//
// Every surface that shows them (the trust bar's strip and its FULL TERMS
// overlay, the money sheets, the hand history, How it works, the Verify code
// dialog) renders these strings. Change a word here and every surface follows.

/** The lead of the one-line strip, rendered bold, followed by a colon. */
export const NOTICE_LEAD = 'Unaudited code with known bugs';

/** The rest of the one-line strip, after "Unaudited code with known bugs:". */
export const NOTICE_STRIP =
  'your funds are NOT safe. Online gambling is illegal in many jurisdictions. 18+ only. '
  + 'No middleman, no house. No rake is taken from any pot on any table.';

/** The canonical no-rake sentence, alone. */
export const NOTICE_NO_RAKE = 'No rake is taken from any pot on any table.';

/**
 * The full terms, paragraph by paragraph. `warningBody` follows the lead
 * sentence ("Unaudited code with known bugs."); `info` carries the no-rake
 * sentence in the middle of a longer statement; `ai` is provenance.
 */
export const NOTICE_TERMS = Object.freeze({
  warningBody:
    'This is for educational and testing purposes only. Any deposit of ICP or Bitcoin is at '
    + 'your own risk: your funds are NOT safe. Expect to lose everything you deposit. Online '
    + 'gambling is illegal in many jurisdictions. Only use where legally permitted. 18+ only.',
  infoBefore: 'No middleman, no house.',
  infoAfter:
    'Built to demonstrate the power of the Internet Computer: 100% on-chain, with the '
    + 'frontend, backend, and game logic all running on smart contracts (canisters). '
    + 'Provably fair, fully transparent, and completely decentralized.',
  ai: 'This entire project was built 100% by AI.',
});

/** The strip as one plain string, for a title or a test. */
export const NOTICE_STRIP_TEXT = `${NOTICE_LEAD}: ${NOTICE_STRIP}`;
