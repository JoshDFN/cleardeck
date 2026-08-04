// Screenshot / frame-burst writing plus the run manifest.

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { ARTIFACTS_DIR, REPO_ROOT } from './config.mjs';

/** Short git sha of the working tree's HEAD (used as the artifact directory). */
export function gitShortSha() {
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
      cwd: REPO_ROOT, encoding: 'utf8',
    }).trim();
  } catch {
    return 'nogit';
  }
}

export function gitDirty() {
  try {
    const out = execFileSync('git', ['status', '--porcelain'], {
      cwd: REPO_ROOT, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
    });
    return out.trim().length > 0;
  } catch {
    return false;
  }
}

export function runDirs(sha) {
  const shaDir = path.join(ARTIFACTS_DIR, sha);
  const latestDir = path.join(ARTIFACTS_DIR, 'latest');
  fs.mkdirSync(shaDir, { recursive: true });
  fs.mkdirSync(latestDir, { recursive: true });
  return { shaDir, latestDir };
}

/**
 * Writes `<scene>-<viewport>.png` at exactly the viewport size, plus a
 * `-full.png` full-page variant (ClearDeck pages are taller than any viewport
 * because of the disclaimer banner and footer).
 */
export async function shoot(page, { scene, viewport, shaDir, latestDir }) {
  const base = `${scene}-${viewport}`;
  const files = [];

  const viewportFile = path.join(shaDir, `${base}.png`);
  await page.screenshot({ path: viewportFile, fullPage: false, animations: 'disabled' });
  files.push(viewportFile);

  const fullFile = path.join(shaDir, `${base}-full.png`);
  await page.screenshot({ path: fullFile, fullPage: true, animations: 'disabled' });
  files.push(fullFile);

  for (const f of files) {
    fs.copyFileSync(f, path.join(latestDir, path.basename(f)));
  }
  return files.map((f) => path.relative(REPO_ROOT, f));
}

/**
 * Captures a burst of viewport stills, for animation/responsiveness review.
 * There is no ffmpeg on this machine, so a numbered frame sequence is the
 * portable artifact; Playwright's own bundled encoder produces the .webm.
 */
export async function burst(page, { dir, frames = 24, everyMs = 150 }) {
  fs.mkdirSync(dir, { recursive: true });
  const written = [];
  for (let i = 0; i < frames; i += 1) {
    const file = path.join(dir, `frame-${String(i).padStart(3, '0')}.png`);
    await page.screenshot({ path: file, fullPage: false });
    written.push(path.relative(REPO_ROOT, file));
    if (i < frames - 1) await page.waitForTimeout(everyMs);
  }
  return written;
}

export function writeManifest(shaDir, latestDir, manifest) {
  const json = JSON.stringify(manifest, (_k, v) => (typeof v === 'bigint' ? v.toString() : v), 2);
  fs.writeFileSync(path.join(shaDir, 'manifest.json'), json);
  fs.writeFileSync(path.join(latestDir, 'manifest.json'), json);
}

/** Human-readable index of the run, written next to the PNGs. */
export function writeIndex(shaDir, latestDir, manifest) {
  const lines = [
    '# ClearDeck screenshot run',
    '',
    `- commit: \`${manifest.gitSha}\`${manifest.gitDirty ? ' (working tree dirty)' : ''}`,
    `- started: ${manifest.startedAt}`,
    `- auth: ${manifest.auth}`,
    `- app origin: ${manifest.appOrigin}`,
    `- gateway: ${manifest.gateway}`,
    `- frontend asset canister: ${manifest.frontendCanisterId}`,
    '',
    '| scene | viewport | file | state verified on-chain | notes |',
    '| --- | --- | --- | --- | --- |',
  ];
  for (const s of manifest.scenes) {
    for (const shot of s.shots) {
      lines.push(
        `| ${s.scene} | ${shot.viewport} | \`${shot.files[0]}\` | ` +
          `${s.verified ? 'yes' : 'NO'} | ${(shot.notes || s.notes || '').replace(/\|/g, '/')} |`,
      );
    }
    if (s.shots.length === 0) {
      lines.push(`| ${s.scene} | - | (failed) | NO | ${(s.error || '').replace(/\|/g, '/')} |`);
    }
  }
  lines.push('');
  const text = lines.join('\n');
  fs.writeFileSync(path.join(shaDir, 'INDEX.md'), text);
  fs.writeFileSync(path.join(latestDir, 'INDEX.md'), text);
  return text;
}
