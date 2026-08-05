// WHAT ELSE WAS RUNNING WHILE THE STOPWATCH WAS RUNNING.
//
// The wave-3 responsiveness numbers were taken on a machine that also had
// several cargo builds and several PocketIC instances on it, because five agents
// share this tree and this laptop. Every latency figure measured that way is
// inflated by an unknown amount, and no amount of care inside the measurement
// can subtract it. A latency document that does not say what the machine was
// doing is a document whose numbers cannot be compared with anything — including
// with itself, next wave.
//
// So the load is SAMPLED, not asserted: the run records the 1/5/15-minute load
// averages and the heavy competing processes at the start, periodically during,
// and at the end, and the JSON carries all of it. "Quiet" then means a number a
// reader can check rather than an adjective the author chose.

import os from 'node:os';
import { execFileSync } from 'node:child_process';

/** Processes whose presence materially changes a latency measurement here. */
// Matched against the full command line, ANCHORED AT THE EXECUTABLE. Two traps
// were hit writing this and both are now closed by the anchoring:
//   * a bare /node .*perf\.mjs/ also matched the `/bin/zsh -c "node
//     tools/shots/perf.mjs …"` wrapper, so a run reported a second harness
//     process that did not exist;
//   * the `comm` column cannot be split on whitespace here, because the managed
//     network's binary lives under "…/Library/Application Support/…".
const HEAVY = [
    { name: 'cargo/rustc', re: /^\S*\/?(cargo|rustc)(\s|$)/ },
    { name: 'pocket-ic', re: /pocket-ic(\s|$)/ },
    { name: 'node (harness)', re: /^\S*node\s+\S*(run|perf|perf-reference)\.mjs(\s|$)/ },
    { name: 'icp-cli', re: /^\S*\/?icp\s/ },
];

/** One reading of what the machine is doing right now. */
export function sampleLoad() {
    const [one, five, fifteen] = os.loadavg();
    const counts = {};
    let processes = '';
    try {
        processes = execFileSync('ps', ['-Ao', 'pcpu=,args='], {
            encoding: 'utf8', maxBuffer: 8 * 1024 * 1024,
        });
    } catch { /* a machine without ps still gets load averages */ }
    // `ps -Ao pcpu=,args=`: percent CPU, then the whole command line. No header,
    // and the command is taken as everything after the first field, so a path
    // containing spaces stays intact.
    const rows = processes.split('\n').filter(Boolean).map((line) => {
        const trimmed = line.trim();
        const cut = trimmed.indexOf(' ');
        return {
            pcpu: Number.parseFloat(trimmed.slice(0, cut)),
            args: cut === -1 ? '' : trimmed.slice(cut + 1),
        };
    });
    for (const { name, re } of HEAVY) {
        counts[name] = rows.filter((r) => re.test(r.args)).length;
    }
    // Total CPU busy-ness as ps sees it, which catches heavy processes this list
    // does not name.
    const totalPcpu = rows.reduce((n, r) => n + (Number.isFinite(r.pcpu) ? r.pcpu : 0), 0);
    return {
        at: new Date().toISOString(),
        loadAvg: { '1m': one, '5m': five, '15m': fifteen },
        loadPerCore: Number((one / os.cpus().length).toFixed(3)),
        heavyProcesses: counts,
        totalPercentCpuAcrossAllProcesses: Number(totalPcpu.toFixed(1)),
    };
}

/**
 * Samples the machine's load for as long as the measurement runs.
 *
 * @param {{everyMs?: number}} [opts]
 * @returns {{stop: () => object}} call stop() to end sampling and get the record
 */
export function startLoadWatch({ everyMs = 10_000 } = {}) {
    const cpus = os.cpus();
    const samples = [sampleLoad()];
    const timer = setInterval(() => samples.push(sampleLoad()), everyMs);
    timer.unref?.();

    return {
        stop() {
            clearInterval(timer);
            samples.push(sampleLoad());
            const ones = samples.map((s) => s.loadAvg['1m']);
            const perCore = samples.map((s) => s.loadPerCore);
            const busiest = samples.reduce(
                (worst, s) => (s.loadAvg['1m'] > worst.loadAvg['1m'] ? s : worst), samples[0],
            );
            const anyHeavy = (name) => samples.some((s) => (s.heavyProcesses[name] || 0) > 0);
            return {
                machine: {
                    platform: `${os.platform()} ${os.release()}`,
                    cpuModel: cpus[0]?.model ?? 'unknown',
                    cpuCount: cpus.length,
                    totalMemGiB: Number((os.totalmem() / 1024 ** 3).toFixed(1)),
                },
                samples,
                sampleCount: samples.length,
                loadAvg1mMin: Math.min(...ones),
                loadAvg1mMax: Math.max(...ones),
                loadPerCoreMax: Math.max(...perCore),
                busiestSample: busiest,
                competingWork: {
                    cargoOrRustcSeen: anyHeavy('cargo/rustc'),
                    extraHarnessProcessSeen: samples.some((s) => (s.heavyProcesses['node (harness)'] || 0) > 1),
                    pocketIcSeenMax: Math.max(...samples.map((s) => s.heavyProcesses['pocket-ic'] || 0)),
                },
                // The verdict a reader should apply to every number in the run.
                // Deliberately mechanical — "quiet" is a threshold, not an opinion —
                // and deliberately compound: a machine can be quiet by load average
                // while ANOTHER screenshot/perf run is driving the same replica, and
                // that second fact matters more to a latency number than the first.
                verdict: [
                    Math.max(...perCore) <= 0.35
                        ? `1-minute load stayed at or under ${Math.max(...perCore).toFixed(2)} per core `
                          + `(peak ${Math.max(...ones).toFixed(2)} across ${cpus.length} cores)`
                        : `NOT QUIET: peak 1-minute load ${Math.max(...ones).toFixed(2)} `
                          + `(${Math.max(...perCore).toFixed(2)} per core)`,
                    anyHeavy('cargo/rustc')
                        ? 'a cargo/rustc BUILD was running'
                        : 'no cargo/rustc build ran',
                    samples.some((s) => (s.heavyProcesses['node (harness)'] || 0) > 1)
                        ? 'ANOTHER harness process (screenshot or perf run) was on the same '
                          + 'replica for part of this run'
                        : 'no other harness process was running',
                ].join('; '),
            };
        },
    };
}
