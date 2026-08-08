// The statistics the rest of the tool is not allowed to skip.
//
// A false collusion accusation is worse than a missed one, and the cheapest way
// to manufacture one is a confident-looking number computed from a handful of
// hands. So every rate this tool prints comes with an interval, every pair test
// is corrected for the fact that many pairs were tested, and every test can
// answer "this sample cannot decide" instead of returning a p-value nobody
// should act on.

/** mulberry32. Every resample in this file is reproducible from its seed. */
export function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6D2B79F5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Wilson score interval for a proportion. Used everywhere a percentage is shown,
 * because "VPIP 40%" off 10 hands and off 10,000 hands are not the same claim and
 * a bare percentage cannot tell them apart.
 */
export function wilson(k, n, z = 1.959963985) {
  if (n === 0) return { p: null, lo: null, hi: null, n: 0 };
  const p = k / n;
  const d = 1 + (z * z) / n;
  const centre = (p + (z * z) / (2 * n)) / d;
  const half = (z * Math.sqrt((p * (1 - p)) / n + (z * z) / (4 * n * n))) / d;
  return { p, lo: Math.max(0, centre - half), hi: Math.min(1, centre + half), n };
}

/**
 * log(n!), summed exactly rather than approximated.
 *
 * Stirling is fine for large n and wrong enough at n < 10 to move a Fisher
 * p-value, and small n is exactly where these tests live. The table grows on
 * demand and is never recomputed.
 */
const LN_FACT = [0, 0];
function lnFactorial(n) {
  if (n < 0) return NaN;
  for (let i = LN_FACT.length; i <= n; i += 1) LN_FACT.push(LN_FACT[i - 1] + Math.log(i));
  return LN_FACT[n];
}

const lnChoose = (n, k) => (k < 0 || k > n ? -Infinity : lnFactorial(n) - lnFactorial(k) - lnFactorial(n - k));

/**
 * Right-tail Fisher exact test on the 2x2 table
 *   [ both, aOnly ; bOnly, neither ]
 * i.e. "did these two appear together more often than their own totals explain".
 */
export function fisherRightTail(both, aTotal, bTotal, n) {
  const lo = both;
  const hi = Math.min(aTotal, bTotal);
  let p = 0;
  const denom = lnChoose(n, aTotal);
  for (let k = lo; k <= hi; k += 1) {
    p += Math.exp(lnChoose(bTotal, k) + lnChoose(n - bTotal, aTotal - k) - denom);
  }
  return Math.min(1, Math.max(0, p));
}

/**
 * The SMALLEST p-value this pair could ever produce, given how often each played.
 * If even a perfect co-occurrence cannot clear the threshold, the test has no
 * power here and reporting a p-value at all would be misleading.
 */
export const minimumAttainableP = (aTotal, bTotal, n) =>
  fisherRightTail(Math.min(aTotal, bTotal), aTotal, bTotal, n);

/** Benjamini-Hochberg q-values, in the input order. */
export function benjaminiHochberg(pvalues) {
  const m = pvalues.length;
  if (m === 0) return [];
  const order = pvalues.map((p, i) => ({ p, i })).sort((a, b) => a.p - b.p);
  const q = new Array(m);
  let prev = 1;
  for (let rank = m; rank >= 1; rank -= 1) {
    const { p, i } = order[rank - 1];
    prev = Math.min(prev, (p * m) / rank);
    q[i] = Math.min(1, prev);
  }
  return q;
}

export const mean = (xs) => (xs.length === 0 ? null : xs.reduce((a, b) => a + b, 0) / xs.length);

/**
 * Right-tail p-value for "the mean of `sample` is higher than a mean of the same
 * size drawn from `baseline`".
 *
 * A bootstrap, not a t-test: per-hand chip flows in poker are wildly
 * heavy-tailed (most hands are zero, a few are the whole stack), and a t-test on
 * that shape produces small p-values out of nothing. This makes no distributional
 * assumption at all.
 */
export function bootstrapMeanPValue(sample, baseline, { iters = 20_000, seed = 12345 } = {}) {
  if (sample.length === 0 || baseline.length === 0) return null;
  const observed = mean(sample);
  const rand = rng(seed);
  const n = sample.length;
  const m = baseline.length;
  let atLeast = 0;
  for (let it = 0; it < iters; it += 1) {
    let s = 0;
    for (let i = 0; i < n; i += 1) s += baseline[Math.floor(rand() * m)];
    if (s / n >= observed) atLeast += 1;
  }
  // +1/+1 so a p-value is never reported as exactly zero from a finite resample.
  return (atLeast + 1) / (iters + 1);
}

/** Percentile bootstrap confidence interval for a mean. */
export function bootstrapMeanCI(sample, { iters = 10_000, seed = 999, alpha = 0.05 } = {}) {
  if (sample.length === 0) return { lo: null, hi: null };
  const rand = rng(seed);
  const n = sample.length;
  const means = new Array(iters);
  for (let it = 0; it < iters; it += 1) {
    let s = 0;
    for (let i = 0; i < n; i += 1) s += sample[Math.floor(rand() * n)];
    means[it] = s / n;
  }
  means.sort((a, b) => a - b);
  return {
    lo: means[Math.floor((alpha / 2) * iters)],
    hi: means[Math.min(iters - 1, Math.floor((1 - alpha / 2) * iters))],
  };
}

/**
 * The standard normal quantile, by Acklam's rational approximation
 * (|error| < 1.15e-9 over the whole range).
 *
 * Written out rather than hardcoded as "1.645 at 5%" because the threshold this
 * tool actually operates at is NOT 5%: it is 5% divided by the number of pairs
 * examined. A power table computed at the uncorrected alpha overstates what the
 * detector can see by a factor of more than two, which would make the "hands
 * needed" numbers in the documentation quietly wrong in the reassuring direction.
 */
export function zQuantile(p) {
  if (!(p > 0 && p < 1)) return NaN;
  const a = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02,
    1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
  const b = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02,
    6.680131188771972e+01, -1.328068155288572e+01];
  const c = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00,
    -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
  const d = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00,
    3.754408661907416e+00];
  const pLow = 0.02425;
  if (p < pLow) {
    const q = Math.sqrt(-2 * Math.log(p));
    return (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
      / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1);
  }
  if (p > 1 - pLow) return -zQuantile(1 - p);
  const q = p - 0.5;
  const r = q * q;
  return (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
    / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1);
}

const zSum = (alpha, power) => zQuantile(1 - alpha) + zQuantile(power);

/** Standard normal CDF (Abramowitz & Stegun 7.1.26 on erf), |error| < 1.5e-7. */
export function normalCdf(x) {
  const t = 1 / (1 + 0.3275911 * Math.abs(x) / Math.SQRT2);
  const y = 1 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
    + 0.254829592) * t * Math.exp(-(x * x) / 2);
  return x >= 0 ? 0.5 * (1 + y) : 0.5 * (1 - y);
}

/**
 * Analytic power of the one-sided test: the probability it flags a leak of
 * `effect` given `n` observations of the variance seen in `baseline`.
 * Printed beside the simulated rate so the two can be compared instead of
 * assumed to agree.
 */
export function analyticPower(baseline, effect, n, { alpha = 0.05 } = {}) {
  if (baseline.length < 2 || n < 2) return null;
  const se = Math.sqrt(sampleVariance(baseline) / n);
  return normalCdf(effect / se - zQuantile(1 - alpha));
}

function sampleVariance(xs) {
  const mu = mean(xs);
  return xs.reduce((a, b) => a + (b - mu) ** 2, 0) / (xs.length - 1);
}

/**
 * The smallest effect this sample could have detected, in the sample's own units.
 *
 * Reported next to every "no signal", because "we found nothing" and "we could
 * not have found anything" are different answers and only one of them is
 * reassuring.
 */
export function detectableEffect(baseline, n, { power = 0.8, alpha = 0.05 } = {}) {
  if (baseline.length < 2 || n < 2) return null;
  return zSum(alpha, power) * Math.sqrt(sampleVariance(baseline) / n);
}

/**
 * Hands needed for a one-sided test at `alpha` to detect `effect` with `power`,
 * given the per-hand variance actually observed.
 */
export function samplesNeeded(baseline, effect, { power = 0.8, alpha = 0.05 } = {}) {
  if (baseline.length < 2 || !effect || effect <= 0) return null;
  const z = zSum(alpha, power);
  return Math.ceil((z * z * sampleVariance(baseline)) / (effect * effect));
}

/** One-sided binomial tail: P(X >= k) with X ~ Bin(n, p). */
export function binomialRightTail(k, n, p) {
  if (n === 0) return 1;
  let total = 0;
  for (let i = k; i <= n; i += 1) {
    total += Math.exp(lnChoose(n, i) + i * Math.log(p || 1e-300) + (n - i) * Math.log(1 - p || 1e-300));
  }
  return Math.min(1, Math.max(0, total));
}
