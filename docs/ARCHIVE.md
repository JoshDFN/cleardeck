# The ClearDeck archive, and what you can do with it

> **Status.** Unaudited alpha software. Nothing in this document is a security
> claim about the poker engine; the known defects are in [DEFECTS.md](DEFECTS.md)
> and [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md). What this document describes
> is an ANALYSIS TOOL over data the table has already published, and the tool's
> own gates are in `tools/archive/selftest/run.mjs`.

---

## 0. The one thing here that a poker room cannot copy

A commercial poker room cannot show you a hand you were not in, and it can never
show you the cards of a player who folded. Not because it is unwilling — because
publishing them would let the next player at the table read the previous one's
range in real time. PokerCraft and Hold'em Manager are real businesses built on
what is left over after that restriction.

ClearDeck reveals the seed when the hand ends, and the seed determines the whole
52-card deck ([SHUFFLE-SPEC.md](SHUFFLE-SPEC.md)). Three things follow, and they
are the reason this tool exists:

| | |
|---|---|
| **Every hand ever played is fully reconstructible by anyone.** | Every hole card of every seat, whether or not it was shown. Measured: on 2,250 real hands from a local archive, 4,947 hole cards that no record publishes were recovered from the seeds. |
| **The run-out is known even for hands that ended early.** | The board comes off the shuffled deck at offsets fixed by `P`, and `P` is fixed when the cards are dealt. Betting cannot move it. So the turn and river of a hand that ended pre-flop are not a guess: they are `deck[2P+5]` and `deck[2P+7]`. |
| **Therefore a fold has an exact price** whenever calling would have closed the action. | No model, no range assumption, no simulation. The cards. |

The third one is the feature. Every other hand-history tool in existence has to
*estimate* what a fold cost you, because it does not know what the other player
had. This one does not have to estimate.

None of that requires trusting us. The tool fetches records and then runs
offline, and the fetch step is separable: hand it a JSON bundle from any source
and it gives the same answers.

---

## 1. What it does, in the order it does it

```bash
# the ONLY networked step
node tools/archive/fetch/fetch-archive.mjs --out /tmp/cleardeck-bundle.json

# everything after this is offline
node tools/archive/bin/cdarchive.mjs verify    /tmp/cleardeck-bundle.json
node tools/archive/bin/cdarchive.mjs hand      /tmp/cleardeck-bundle.json --id 181 --trace --ev
node tools/archive/bin/cdarchive.mjs stats     /tmp/cleardeck-bundle.json
node tools/archive/bin/cdarchive.mjs ev        /tmp/cleardeck-bundle.json
node tools/archive/bin/cdarchive.mjs collusion /tmp/cleardeck-bundle.json
```

The analysis half of `tools/archive` has **no dependencies at all** — `node:crypto`
and nothing else — and shares no line with `poker_core`, the canister, or
`src/declarations`. That is not tidiness. A tool that reuses ClearDeck's own
shuffle can never catch ClearDeck's shuffle being wrong, and a tool that reuses
ClearDeck's own evaluator can never catch a showdown being misjudged.

### `src/declarations/history` cannot see the field that makes this possible

`fetch/` carries its own Candid mirror of the archive canister. It has to:
`src/declarations/history/history.did.js` is generated, stale, and **missing
`dealt_in`** — on `HandHistoryRecord` and on `PlayerHandRecord` — plus
`contributed`, `left_mid_hand` and `HandSummary.dealt_in_count`. Candid record
subtyping drops fields the decoder does not declare, silently, so a client built
from those declarations fetches happily and simply never sees `dealt_in`. It
would then have to guess `P`, which is
[FINDING 30](SECURITY-FINDINGS.md#finding-30): the hole cards still match at any
`P` while the board silently comes out wrong. `history.did` next to it *does*
carry the field, so the drift is in the generated client, not the interface.
`fetch/history-wire.mjs` asserts the field is present at startup and refuses to
run without it.

---

## 2. Reconstruction, and how it shows its work

`cdarchive hand <bundle> --id N` prints every intermediate value, in the order
[SHUFFLE-SPEC.md](SHUFFLE-SPEC.md) puts them, so the output is checkable rather
than asserted:

0. **`SHA256(revealed_seed) == seed_hash`**, computed locally. A failure here
   names the mistake — transposed fields, a truncated paste, an empty field —
   instead of returning a bare `false` a player would read as "I was cheated".
1. **The whole 52-card deck**, with `--trace` printing the hash chain step by
   step: `chain`, the little-endian draw, the rejection count, `j = draw mod n`.
2. **`P`, READ from `dealt_in`**, never counted. With the offsets it implies
   spelled out: `flop deck[2P+1..2P+3]`, `turn deck[2P+5]`, `river deck[2P+7]`.
3. **Every card the record published, beside the card the seed produces**, with
   the deck index each came from and an explicit agrees/NO.
4. **The cards no record publishes**, recovered from the seed, labelled as
   derived so nobody mistakes one for a published card.
5. **Whether the check is vacuous**: what the board would have been at `P-1` and
   `P+1`. If the neighbouring `P` gives the same board, the tool says the board
   check does not bind on that hand.
6. **The betting replayed**, with the uncalled bet returned where it was
   returned, and the money cross-checked four ways.
7. **The showdown scored by this tool's own evaluator**, compared against the
   `final_hand_rank` the canister recorded. A disagreement is a non-zero exit.

**If anything disagrees, the hand is unusable.** It is excluded from every
statistic, from EV, and from the collusion ledger, and the exclusion is printed
with its reason. A statistic computed over a set that quietly includes the hands
the tool could not check is precisely the instrument this repository keeps
finding: correct totals, wrong recipients, every invariant silent.

### The one card in the output with nothing to check it against

A showdown hand can be compared against what the record published. **A folded
hand cannot** — and a folded hand is the product. It rests entirely on the mapping
`dealt_in[k] -> deck[2k], deck[2k+1]`. If that mapping were permuted, every hand
where only one seat showed would still verify perfectly and every folded hand
would come out wrong, silently, forever.

Two things address it, and neither is an argument:

* `cdarchive verify` reports **how many hands published two or more hands**, which
  is what pins the order. On the 2,250-hand run below, 757 did. On the other 1,493
  the deal order is assumed rather than checked, and the output says so rather than
  letting a reader assume it was tested.
* `tools/archive/session/verify-live.mjs` **watches the deal happen.** It plays
  real hands, reads every seat's own view of its own two cards while the hand is
  live — the canister shows a player their own cards whether or not they later
  fold — then reconstructs from the archive and compares. Measured, on 45 real
  hands:

```
hands matched to a live observation      45
hole cards compared                     180
of those, cards NO RECORD PUBLISHES     156
DISAGREEMENTS                             0
```

156 cards that appear in no record, reconstructed from the seed, checked one by
one against what the canister actually put in each player's hand.

### Measured, on real hands from real canisters

`./scripts/dev.sh local-up` and a series of scripted sessions produced 2,250
settled hands across three tables. Reconstructing all of them from their seeds
alone, and cross-checking the money in each:

```
hands VERIFIED                 2250
hands UNVERIFIABLE                0     (no seed, or the record does not state P)
hands the record CONTRADICTS      0
individual card checks        11267
card DISAGREEMENTS                0
hole cards reconstructed that the record never published   4947
hands where two or more seats published, which pins the deal order    757
hands whose money checks out   2250
hands whose money does NOT        0
```

---

## 3. The money, and the four checks it has to survive

The cards come from the seed. The **money does not** — chip flows exist only in
the record — so the record is the primary source for them and the job is to
re-derive the same numbers a second way and see whether the record agrees with
itself. Five statements of the same fact are available:

| check | what it would catch |
|---|---|
| `sum(contributed) == total_pot` | money in the pot that nobody put there |
| `sum(winners) == total_pot` | money paid out that nobody won |
| `starting - contributed + won == ending` | a stack that does not follow from the hand |
| `rake == 0` | **the no-rake property, hand by hand, permanently** |
| the replay from blinds + actions == `contributed` | the action log disagreeing with the settlement |

Three things had to be got right for the fifth to hold, and each was wrong first:

* **`Raise(x)` means raise TO x**, not raise BY x, and `AllIn(x)` records the
  shover's street total after the shove. Reading either as an increment inflates
  every pot. Pinned by a gate that reads it the wrong way on a real hand and
  lands somewhere else.
* **The uncalled bet comes back.** A player who bets 0.18 into a 0.30 pot and
  takes it uncontested wagered 0.18 and *contributed* only what somebody matched.
  Before this was modelled, 5 of 22 real hands looked like the record did not add
  up — a tool that would have accused the canister of losing money it had
  correctly returned.
* **In a heads-up hand the dealer is the small blind, and the record labels that
  seat `BTN`.** There is no `SB` label to find. A derivation that expected one is
  wrong on every heads-up hand in the archive.

### What this found: a big blind that costs half a big blind

The fifth check disagreed with the record on exactly one hand out of 2,250, and
the record turned out to be right. Archived hand 604: the big blind posted
0.10 ICP and the record says they contributed **0.05**.

`leave_table` hands a departing player back anything of theirs nobody had covered
— the uncalled-bet rule, applied on the way out. **Immediately after the blinds the
big blind is always the top contributor**, so a player who posts the big blind and
leaves before the action reaches them gets `big_blind - small_blind` of their own
posted blind returned. Reproduced on a live local table, four times out of four,
at a 1:2 structure: **the big blind cost 0.01 ICP instead of 0.02, every time.**

Nothing about that is a loss to anybody else. The pot balances to the e8, the
winner is paid what is there, and every sum in the record is right. What moves is
the **price**: the other players still had to call the full big blind into a pot
that no longer contains one, and the small blind's fold-out pot is smaller than the
blinds it was built from. In every cardroom rulebook a posted blind stays in the
pot when you leave.

This is the same silence the repository keeps running into from a new direction.
Correct totals; nobody robbed; and a repeatable economic edge that no invariant
over sums can see. `cdarchive verify` now reports it under its own heading rather
than either failing the hand or hiding it, and `session/verify-live.mjs` is not
needed to see it: it is in the archive, in public, for anyone who looks.

**Not exercised:** antes. Both local tables run `ante = 0`, so the ante path in
the replay is written from the canister's posting order and has never been run
against a real hand that used it.

---

## 4. Statistics

`cdarchive stats` gives VPIP, PFR, 3-bet, post-flop aggression factor and
frequency, WTSD, W$SD, WWSF, bb/100, and all of it broken down by position.

Two rules, both of which most tools break:

* **Every rate carries the count it came from and a 95% Wilson interval.** "VPIP
  40%" off 10 hands and off 10,000 hands are not the same claim and a bare
  percentage cannot tell them apart. Below `--min-hands` (default 100) the rate is
  printed with the word `noise` next to it.
* **Denominators are facts about the hand, not about the action log.** "Saw a
  flop" means the flop was dealt and this player was still in it — *not* "acted
  after the flop". A player who is all-in pre-flop takes no post-flop action and
  still sees every card, and counting the denominator from actions would quietly
  drop exactly the hands where the most money was at stake.

---

## 5. EV, and where the tool stops

Three different numbers, deliberately not blended, because they answer three
different questions and only two of them are exact.

### 5.1 The exact price of a fold  *(the differentiator)*

A fold is priced **only** when calling would have closed the action, so nothing
has to be assumed about what anyone would do next. Three shapes qualify, each
checked against the record rather than inferred:

* the board was complete, the fold ended the hand, and everyone still in had
  already matched — calling goes straight to a showdown;
* every remaining player was already all-in and the fold ended the hand — the
  board just runs out;
* one opponent, and calling would have put the folder all-in.

Then: call `min(toCall, stack)`, return the bettor's excess if the call could not
cover it, run the board out **from the deck the seed already fixed**, layer the
pots and settle. The answer is `wouldWin - callAmount` and it is exact.

Anything else prints **"no closed counterfactual"** and no number. That is the
line most tools cross: the price of a fold when calling would have left more
decisions to make depends on what everyone would have done next, which is not in
the record and is not knowable. A tool that prints one is printing an opinion.

*Caveat, stated because it is real and small:* when a counterfactual pot chops and
does not divide evenly, this tool gives the odd e8 to the lowest seat index while
the canister gives it clockwise from the button
(`poker_core::split_pot_clockwise`). The two agree on the total and can differ by
one e8 — 10⁻⁸ ICP — on a chopped odd pot.

### 5.2 Hindsight equity

At any decision, that player's exact equity against the **actual** holdings of
everyone still in, over the run-outs that were still unseen.

This is **not** a measure of how well the decision was played — nobody could see
those cards. It is printed with that sentence attached every time, because it is
the number a colluder effectively *does* see, and that is what makes it the right
input to §6.

### 5.3 All-in EV, i.e. luck

When the betting finished before the board did, what each pot layer was worth on
equity versus what the cards actually paid. The difference is the run-out, not the
play, and the output says so.

**The randomisation is stated, because a counterfactual without one means
nothing.** Equity is taken uniformly over the cards *not visible to the players at
that moment* — the 52 minus the board so far minus the contesting hole cards.
Folded cards and burns stay in the pool: the players could not see them either.
Enumeration is exact wherever the board can be enumerated (up to 1,712,304
run-outs); above the budget the tool samples and prints `sampled` with a standard
error rather than pretending.

---

## 6. Collusion signals

> ## READ THIS BEFORE ACTING ON ANY VERDICT IN THIS SECTION
>
> **Signal 2 cannot tell an honest winning player from a chip dumper, and it was
> published as though it could.** A wave-12 critic played **800 hands on the real
> local `table_3` with three bots that never coordinate and never see each other's
> cards** — a calling station, the repo's own honest bot, and a tight-aggressive
> winner. The shipped detector returned **two `REVIEW` rows at q = 1.50e-4, both
> naming the winner as the beneficiary**. The deliberately colluding session
> returns **two `REVIEW` rows at q = 1.50e-4**. Same verdict, same count, same q,
> same table shape. Nothing in the output distinguishes them.
>
> **The null hypothesis is false under honest play, and that is a design fault,
> not a tuning problem.** Signal 2 asks "does A lose to B faster than A loses to
> everybody else". At a table where opponents differ in skill, that is also the
> definition of "B is better than A's other opponents". Under a false null the
> error rate **rises toward 1 with sample size** instead of staying at alpha:
> holding an ordinary 60 bb/100 skill spread fixed and growing the sample, the
> flag rate goes 0/10 → 1/10 → 0/10 → **4/10** at 700 / 2,252 / 6,592 / 19,629
> shared hands.
>
> **The measured false-positive rate below does not bound this.** `selftest/
> synthetic.mjs` draws every player's per-hand result from one and the same
> `drawResult()`: it has no per-player skill parameter at all, so its 25 "honest"
> worlds contain **zero skill variation**, which is the entire economy of a poker
> room. Add a skill ladder and a 200 bb/100 spread flags 5 of 25 honest
> populations. The assertion behind the sentence "at or below the stated alpha" is
> also `assert(flagged / trials <= 0.12)`, which passes at 2.4× the alpha its own
> name and message claim.
>
> **And the p-value underneath is anti-conservative by construction**: it omits
> the sample's own sampling variability, so on perfectly Gaussian data with no
> tail it fires at 14% where it claims 5%, and at 2.4% where it claims 0.167%.
>
> **What this section is therefore worth today.** Signal 1 (co-occurrence) and
> Signal 3 (folding the winner to one opponent, priced exactly) are sound and are
> the ones with a mechanism no room outside this one has. Signal 2 is a
> **descriptive statistic that must not be read as evidence**: a `REVIEW` row from
> it means "money moved in this direction", not "this is anomalous". Until the
> baseline is conditioned on opponent strength — a skill-adjusted null, not a
> tighter alpha — **no Signal 2 row should be shown to a player or used to
> restrict an account.** Filed as [DEFECTS.md T-42](DEFECTS.md#t-42).

**Automation is not what hurts a poker player. Collusion is.** A room that is
about to allow bots owes its players a detector, and the detector has to be one an
outsider can run or it is just another thing to take on trust.

**A false accusation is worse than a missed one.** So:

* every test states its sample size and **refuses below it**;
* every test is Benjamini-Hochberg corrected for the number of pairs examined —
  looking at 40 pairs and reporting the most extreme one at p<0.05 finds
  "collusion" in honest play about two attempts in three;
* a test that **cannot** produce a significant answer given the shape of the data
  says `NO POWER`, instead of a reassuring `NO SIGNAL` it never earned;
* the strongest verdict any signal can return is **`REVIEW`**, which means "a
  human should look at this hand set". Nothing in the output says a person
  cheated.

### Signal 1 — seat co-occurrence beyond chance

Fisher's exact test on "were these two in the same hand", corrected across pairs.

The trap it exists to avoid: **at a two-seat table everybody co-occurs with their
opponent 100% of the time, by construction**, and a naive test reports every pair
as astronomically significant. So before any p-value is computed the tool asks
whether the test could distinguish anything at all, and refuses when:

* fewer than 6 distinct players are in the sample;
* every player in the sample could be at one table at once;
* **almost nobody moves between tables** — if each player only ever appears at one
  table, who sits with whom was decided by the roster and not by anybody, and the
  test is measuring the roster.

That last refusal fires on ClearDeck's own local sessions, and it is the correct
answer there. Co-occurrence is the weakest of the three signals and is worth
having only in a population where players actually choose their tables.

### Signal 2 — directed chip transfer against the player's own baseline

For each ordered pair, the per-hand net chip flow from A to B in big blinds,
compared against **A's own** flow to every other opponent.

Against A's own baseline, not the field's: *"this player loses money"* is not
collusion, and a detector that fires on it will spend its life accusing bad
players. Collusion looks like a loss that is **directed** — normal against
everyone else, anomalous against one.

> **That reasoning is right and it is not enough, which is the correction at the
> top of this section.** "Directed" removes the *loser's* skill from the
> comparison and leaves the *opponents'* skill in it. A losing player at a table
> with one strong opponent and two weak ones loses to the strong one faster than
> to the others for a reason that has nothing to do with collusion, and this
> signal reports that as a directed anomaly. Measured on 800 real, uncoordinated
> canister hands: two `REVIEW` rows, both accusing the best player, at the same q
> as the scripted dumper.

The attribution rule is stated so it can be argued with: in a hand, each loser's
loss is split across the winners in proportion to what each winner netted. It
conserves chips exactly and is symmetric in the winners; that is the most that can
be claimed for it.

The test is a **bootstrap**, not a t-test. Per-hand poker results are mostly zero
with occasional whole stacks, and a t-test on that shape manufactures significance.

*One approximation, stated:* the sample and its baseline are drawn from
overlapping hands — in a three-way pot, A's flow to B and A's flow to C come out
of the same hand and are negatively correlated by conservation — so the
bootstrap's independence assumption is not exactly right. ~~What bounds the
consequence is not an argument, it is the measured false-positive rate: 25
independent honest populations, built to have exactly this correlation structure,
produced zero `REVIEW` verdicts.~~

**That last sentence is withdrawn.** The 25 populations bound nothing, because
`synthetic.mjs` gives every player the same result distribution and therefore
contains no skill variation to be confused with a directed leak; and the bootstrap
p-value it is computed from omits the sample's own sampling variability, so it is
anti-conservative before any of this. See the block at the top of §6.

> **THE BASELINE CAN BE THE COLLUSION, and this tool found that out on its own
> output.** Pointed at the scripted three-handed table, it flagged the chip dump
> — and it also flagged the *beneficiary* against the innocent third player, at
> q = 1.5e-3. The arithmetic is right and the second row is an artifact: at a
> three-handed table "A's own baseline" is a single relationship, and when that
> relationship is the dump, the baseline is poisoned. Nothing in the numbers says
> which of the two rows is real.
>
> The row is not suppressed — on a wide pool the same arithmetic is sound — but
> every row whose baseline rests on **one** opponent now carries that caveat in
> its own output, and three or more distinct opponents are needed before the
> comparison stands on its own. Pinned by a gate that reproduces the artifact from
> a synthetic three-handed population.

### Signal 3 — folding the winner, to one specific opponent

**The signal an operator normally has and nobody else does — and here anybody has
it.** Among folds whose price is exact (§5.1), how often did A fold a hand that
would have won, and who was on the other side of it?

A strong player wins by betting when they are ahead. They do not win by their
opponent folding the best hand at an anomalous rate. That is what separates skill
from a chip dump in a way pure chip flow cannot.

Baseline, in order of preference: **the folder's own rate against everybody else**
(directed, immune to "this player is just loose); failing that, the population
rate, clearly labelled, with the caveat printed in the row that a player who folds
the winner too often against *everyone* would look identical.

### The sample sizes, measured

One hand of poker is almost all noise. Measured on the **whole archive the
documented fetch command produces** (2,250 hands, 13,172 directed observations),
the per-hand directed chip flow has mean 0 (chips are conserved) and a **standard
deviation of 12.25 big blinds per hand**. `node tools/archive/selftest/power.mjs
<bundle>` prints the table for whatever data you point it at; on that data:

| directed leak | hands the pair must share, at 5% | at the corrected threshold the tool actually uses |
|---|---|---|
| 1 bb/100 | 9,285,109 | 21,422,570 |
| 5 bb/100 | 371,405 | 856,903 |
| 25 bb/100 | 14,857 | 34,277 |
| 50 bb/100 | 3,715 | 8,570 |
| 100 bb/100 | 929 | 2,143 |

> **WAVE-12 CORRECTION.** This table first shipped with `sd = 3.70` and hands-needed
> of 1,957,518 / 78,301 / 3,133 — about **eleven times too optimistic**. 3.70 comes
> from a ~351-hand single-table slice that no documented command produces;
> `power.mjs` takes only a bundle path, with no `--table` or `--limit`, so the
> "honest-session bundle" the claim was `provedBy` is not something a reader can
> make. Re-derived above by running the shipped script on the bundle the documented
> command actually yields. On a later 2,381-hand archive it is sd 16.76 and 1.6M
> hands for a 5 bb/100 leak, so the true figure is **variance-dependent and larger
> than either**. The direction of the correction is the important part: this
> detector is **weaker** than the numbers first published here, not stronger.

`power.mjs` then runs the **shipped** test against that table, so the prediction
is checked rather than believed:

| shared hands | leak | the table predicts | the shipped test achieved |
|---|---|---|---|
| 150 | 0 | 0% | **0%** |
| 150 | 100 bb/100 | 64% | 50% |
| 800 | 100 bb/100 | 100% | 100% |
| 800 | 0 | 0% | **0%** |
| 3,200 | 25 bb/100 | 81% | 80% |
| 3,200 | 0 | 0% | **3%** |

The rows with a leak of zero are the false-positive rate, and they are the only
reason the other rows are worth reading.

Two things to take from that table, and they are the honest ones:

1. **A co-occurrence or chip-flow result over 20 hands means nothing.** Not "is
   weak" — means nothing, against a per-hand standard deviation of 3.7 bb. The
   tool refuses to print one.
2. **A subtle leak is undetectable at any realistic sample.** This detector finds
   gross chip dumping. It does not find a careful pair sharing hole-card
   information for a 3 bb/100 edge, and no chip-flow detector does. The number of
   hands that would take is in the table: about two million.

The table is a normal approximation and the shipped test is a bootstrap, so on
heavier-tailed data the test falls short of the table (on a deliberately
heavy-tailed synthetic population it reaches only about two thirds of the
predicted power). Where they diverge, believe the simulation.

---

## 7. Demonstrated, on real canister hands

`./scripts/dev.sh local-up`, then two scripted sessions playing real hands through
the real table canister and the real settlement path. The scripted players only
choose actions, exactly as a browser would. **One table honest. One table with a
pair where one player folds the river to the other whatever it is holding** — a
chip dump, by construction — and the identical policy against everybody else, so
the transfer is DIRECTED rather than just bad play.

Both windows are **647 hands**: the same sample size, on purpose.

### The archive as a whole

2,250 hands. Every card reconstructed from its seed:

```
hands VERIFIED                 2250          card DISAGREEMENTS         0
hands UNVERIFIABLE                0          individual card checks 11267
hands the record CONTRADICTS      0
hole cards reconstructed that the record never published   4947
hands whose money checks out   2250          hands whose money does NOT   0
```

### Signal 2 — directed chip transfer, at 647 hands each

| | colluding table | honest table |
|---|---|---|
| ordered pairs tested | 6 | 6 |
| **the scripted dumper → partner** | +201.6 bb/100 to them, **+220.3 over their own baseline**, q = 1.5e-4 → **REVIEW** | — |
| largest excess on any pair | +220.3 bb/100 | **+23.1 bb/100** |
| smallest q | 1.5e-4 | 0.76 |
| verdicts | 2 × REVIEW (see below) | **6 × NO SIGNAL** |
| what the sample could have detected | ≥ 52–58 bb/100 | ≥ 40–62 bb/100 |

The honest table was not merely unflagged: the tool states, per pair, the leak it
would have caught, and the honest pairs are an order of magnitude below it.

The second `REVIEW` on the colluding table is the beneficiary against the innocent
third player, and it is the artifact §6 describes — at three-handed, the
beneficiary's "own baseline" *is* the dump. Both rows carry the
single-opponent-baseline caveat in the output.

### Signal 3 — folding the winner, at 647 hands each

Only folds whose price is exact are counted, and the tool **refuses to attach a
p-value to any of these**: nobody has enough exactly-priced folds against a second
opponent to give the folder a baseline of their own. What it does print is the
count, and the count is the whole story:

| | colluding table | honest table |
|---|---|---|
| most concentrated folder | 207 priced folds against one opponent, **99% of all 210 they made** | 14 against one opponent, 74% of 19 |
| how often that fold was the winner | 46.4% [39.7, 53.2] n=207 | 28.6% [11.7, 54.6] n=14 |
| big blinds given up in those folds | **2,263.8** — 100% of everything they gave up | 30.4 |
| verdict | INSUFFICIENT DATA (no baseline) | INSUFFICIENT DATA (below the 40-fold floor) |

Two numbers separated by a factor of 74, and the tool still will not call it
collusion, because a rate with no baseline is not a test. That is the discipline
working, not failing: the count is evidence a human can act on, and the absence of
a q-value says exactly how much arithmetic is behind it.

### Signal 1 — co-occurrence

`NO POWER`, on both tables, for three stated reasons: three distinct players, all
of whom can be at the table at once, none of whom ever plays a second table. On a
fixed roster, who sits with whom was chosen by the roster. The synthetic gates in
`selftest/run.mjs` show the same test firing on a glued pair in a 14-player,
6-table pool and staying silent on twelve honest pools of the same shape.

### The statistics on a real session

647 honest hands, three players, blinds 0.05/0.10 ICP:

```
player     hands  VPIP                      PFR                      AF    bb/100
75qvt…uqe    647  34.6% [31.1, 38.4] n=647  0.9% [0.4, 2.0] n=647    1.97   -17.3
ougln…lae    647  35.7% [32.1, 39.5] n=647  1.9% [1.1, 3.2] n=647    1.97   -16.9
wufmv…yae    647  35.4% [31.8, 39.2] n=647  0.9% [0.4, 2.0] n=647    1.48    34.2
```

The positional breakdown is the sanity check that the numbers mean what they say:
every player's VPIP is ~1–4% in the big blind (there is nothing to add
voluntarily), ~70% in the small blind (completing is cheap) and ~32% on the
button. Those are the shapes poker produces, out of an engine that was never told
about them.

## 8. The gates

A gate nothing runs is worse than no gate.

> **AND FOR ONE WAVE, NOTHING RAN THIS ONE.** `tools/archive/**` shipped with the
> 39 cases below, a sentence in this section saying they held everything above, and
> **no `dev.sh` target, no `make` rule and no CI job that executed a single line of
> it** — in the section that opens with that sentence. Wired in wave 12
> ([DEFECTS.md H-50](DEFECTS.md#h-50)): `./scripts/dev.sh archive` (or
> `make archive`) runs it, and `./scripts/dev.sh test` runs it as step 7 of 7. It
> needs about 85 s, no replica and no network.

`node tools/archive/selftest/run.mjs` is the one that holds everything above — no
replica, no network, 39 cases, each with an answer known before the code runs:

| what it pins | how |
|---|---|
| this tool's shuffle == the canister's | replays all 2,000 committed golden vectors |
| this tool's shuffle == the document | reproduces SHUFFLE-SPEC §5 card for card, including the seed's own hash |
| the deck order and the rejection bound | against the constants in SHUFFLE-SPEC §2 and §3.3 |
| the evaluator | counts all 2,598,960 five-card hands into nine categories and checks every count against its textbook total |
| the fast 7-card path | 30,000 random hands scored again by a slower best-of-21 path written separately |
| equity | enumerates exactly `C(pool, cards to come)` boards, sums to 1, and agrees with the slow scorer board for board |
| side pots | a 10/50/100 three-way all-in, to the chip; a folded contributor funds layers and wins none |
| the reconstruction is not vacuous | one altered card is caught and named; a wrong seed stops at step 0; a null `dealt_in` refuses to guess `P`; the neighbouring `P` gives a different board |
| the betting replay is not vacuous | reading `Raise` as an increment on a real hand lands on a different contribution |
| the uncalled-bet rule | a real hand where the gross wagered exceeds the pot |
| **a pot paid to the wrong person** | one credit moved from the winner to a loser, every sum left balancing to the e8. The winner list and the player list are two independent statements of who was paid, and they have to name the same people |
| the blind-refund allowance | the one place a replay disagreement is forgiven, pinned three ways: it fires on the real hand, it does NOT fire on a seat that stayed, and it does NOT fire when the record disagrees with itself |
| the deal order | a hand where only one seat showed still VERIFIES, and says so, because its folded hands rest on an order nothing checked |
| the boundary | a `nat64` above 2⁵³ is refused rather than rounded |
| ~~**the collusion false-positive rate**~~ | ~~25 independent honest populations, none flagged; 12 more for co-occurrence, none flagged~~ **THIS ROW DOES NOT HOLD WHAT IT SAYS.** The 25 populations have no per-player skill parameter, so they contain none of the variation Signal 2 mistakes for a leak, and the assertion tolerates `flagged/trials <= 0.12` while claiming alpha 0.05. The 12 co-occurrence populations are unaffected. See §6 and [T-42](DEFECTS.md#t-42) |
| the collusion true-positive rate | a scripted dumper and a glued pair are both found |
| the single-opponent baseline artifact | a three-handed synthetic population reproduces the spurious second REVIEW, and every such row carries the contamination caveat |
| the refusals | below every floor, no p-value is produced and the verdict is `INSUFFICIENT DATA` with the missing sample named |
| the statistics | Wilson brackets and narrows; BH is monotone and never below the raw p; Fisher matches a hand-computable 1/252 |

---

## 9. What this does not do

* **It does not audit ClearDeck.** Reconstructing the cards proves the deal
  matches the seed the table committed to. It proves nothing about settlement,
  custody, or the [known defects](DEFECTS.md).
* **It cannot prove the commitment existed before the cards did.**
  `start_new_hand` commits and deals in one message. SHUFFLE-SPEC §0 says so and
  this tool repeats it in the output of every hand it verifies.
* **It cannot see hands the archive does not have.** The table keeps 100 hands and
  a controller can erase them; the archive keeps everything for the life of the
  canister and a controller can delete the canister. `get_history_status` and
  `get_retention_policy` are the two calls that tell you whether anything durable
  is being written *right now* — SHUFFLE-SPEC §6a.
* **It cannot price every decision.** See §5.1.
* **It cannot price ANY decision in a hand somebody walked out of.** Leaving is a
  fold that leaves no trace in the action list, so *when* it happened is not
  recoverable; every counterfactual in such a hand would be built on a table
  containing a player who was no longer at it. Those hands are excluded from EV by
  name, and still counted in the statistics, where only the settled contributions
  matter and those are exact.
* **It cannot catch a careful colluder.** See §6.
* **It cannot tell a strong player from a chip dumper.** Signal 2's null is false
  whenever opponents differ in skill, which is always. Read the block at the top
  of §6 before showing any Signal 2 row to anybody. [T-42](DEFECTS.md#t-42).
* **It has never been run against a hand with an ante.** See §3.
* **It is not a replacement for the money-safety suite.** Its money checks are
  cross-checks over what the archive already published; a hand the table never
  archived is a hand this tool cannot see at all. `get_history_status` is the call
  that says whether that is happening.
