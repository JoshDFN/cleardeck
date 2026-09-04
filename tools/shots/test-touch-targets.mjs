// Self-check for the touch-target fold: node tools/shots/test-touch-targets.mjs
//
// The fold decides whether a scene's "every control is tappable" claim means
// anything, so its failing cases are pinned here without a browser: a small
// box with a grown hit area passes, a small box without one fails, a big box
// covered by a neighbour fails, an empty measurement fails, a type floor
// breach fails, a wide document fails, and an unlocked dialog fails.

import assert from 'node:assert/strict';
import { foldTouchTargets } from './lib/touch-targets.mjs';

const base = () => ({
  viewport: { w: 390, h: 844 },
  coarsePointer: true,
  documentScrollWidth: 390,
  documentScrollHeight: 844,
  scrollLocked: false,
  dialogOpen: false,
  rotatePromptVisible: false,
  items: [],
  type: [],
});

const item = (over = {}) => ({
  path: 'div > button.x', text: 'X', tag: 'button', type: null,
  box: { x: 10, y: 10, w: 44, h: 44, right: 54, bottom: 54 },
  inViewport: true, probed: true, hits: 'yyyyy', boxOk: true, effectiveOk: true, disabled: false, ok: true,
  ...over,
});

let n = 0;
const check = (name, fn) => { fn(); n += 1; console.log(`  ok  ${name}`); };

check('an empty measurement fails, not passes', () => {
  const v = foldTouchTargets(base(), { scene: 's', viewport: 'mobile' });
  assert.equal(v.ok, false);
  assert.match(v.problems[0], /no interactive element/);
});

check('a 44x44 box whose probes all hit passes', () => {
  const m = { ...base(), items: [item()] };
  assert.equal(foldTouchTargets(m, { scene: 's', viewport: 'mobile' }).ok, true);
});

check('a 30 px box grown by a hit area (probes hit) passes', () => {
  const m = { ...base(), items: [item({ box: { x: 10, y: 10, w: 70, h: 30, right: 80, bottom: 40 }, boxOk: false })] };
  assert.equal(foldTouchTargets(m, { scene: 's', viewport: 'mobile' }).ok, true);
});

check('a 30 px box with nothing growing it fails and says so', () => {
  const m = { ...base(), items: [item({ box: { x: 10, y: 10, w: 70, h: 30, right: 80, bottom: 40 }, boxOk: false, hits: 'yyynn', effectiveOk: false, ok: false })] };
  const v = foldTouchTargets(m, { scene: 's', viewport: 'mobile' });
  assert.equal(v.ok, false);
  assert.match(v.problems[0], /under the 44 px floor/);
});

check('a big box covered by a neighbour fails as "covered"', () => {
  const m = { ...base(), items: [item({ hits: 'yyyny', effectiveOk: false, ok: false })] };
  const v = foldTouchTargets(m, { scene: 's', viewport: 'mobile' });
  assert.equal(v.ok, false);
  assert.match(v.problems[0], /covered by a neighbour/);
});

check('an off-viewport small box fails on its box alone', () => {
  const m = { ...base(), items: [item({ box: { x: 10, y: 900, w: 70, h: 30, right: 80, bottom: 930 }, inViewport: false, probed: false, hits: null, boxOk: false, effectiveOk: false, ok: false })] };
  const v = foldTouchTargets(m, { scene: 's', viewport: 'mobile' });
  assert.equal(v.ok, false);
  assert.match(v.problems[0], /off viewport, box only/);
});

check('a type floor breach fails', () => {
  const m = { ...base(), items: [item()], type: [{ label: 'seat name', selector: '.x', minPx: 11, count: 3, smallestPx: 9.5, ok: false }] };
  const v = foldTouchTargets(m, { scene: 's', viewport: 'mobile' });
  assert.equal(v.ok, false);
  assert.match(v.problems[0], /seat name renders at 9.5 px/);
});

check('a document wider than the viewport fails', () => {
  const m = { ...base(), items: [item()], documentScrollWidth: 425 };
  const v = foldTouchTargets(m, { scene: 's', viewport: 'mobile' });
  assert.equal(v.ok, false);
  assert.match(v.problems[0], /horizontal scroll/);
});

check('an open dialog without the scroll lock fails when the lock is expected', () => {
  const m = { ...base(), items: [item()], dialogOpen: true, scrollLocked: false };
  assert.equal(foldTouchTargets(m, { scene: 's', viewport: 'mobile', expectDialogLock: true }).ok, false);
  assert.equal(foldTouchTargets({ ...m, scrollLocked: true }, { scene: 's', viewport: 'mobile', expectDialogLock: true }).ok, true);
});

check('the rotate prompt is asserted both ways when asked', () => {
  const m = { ...base(), items: [item()], rotatePromptVisible: true };
  assert.equal(foldTouchTargets(m, { scene: 's', viewport: 'landscape', expectRotatePrompt: true }).ok, true);
  assert.equal(foldTouchTargets(m, { scene: 's', viewport: 'mobile', expectRotatePrompt: false }).ok, false);
});

console.log(`\ntouch-targets fold: ${n} cases passed`);
