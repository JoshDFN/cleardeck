<script>
  // THE ONE-LINE NOTICE, AS A FRAGMENT.
  //
  // No wrapper element: the caller owns the paragraph and its styling (a
  // money sheet's `.player-notice`, the history's `.legal-warning`, the trust
  // bar's `.strip-text`). This only puts the canonical words inside it, so
  // the five protected phrases are typed once (lib/notices.js) and rendered
  // everywhere from that one source.
  import { NOTICE_LEAD, NOTICE_STRIP } from '../notices.js';

  const {
    /** What follows the bold lead: ':' for the strip, '.' for a heading. */
    punctuation = ':',
  } = $props();
</script>

<strong>{NOTICE_LEAD}{punctuation}</strong> {NOTICE_STRIP}
