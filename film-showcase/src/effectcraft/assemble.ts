// Phase 2: composites the recordings into the film inside EffectCraft (the local
// After Effects-style compositor this pipeline already runs com scripting — not a REST API).
//
// What this script does:
//   1. builds the film plan (the 8 storyboard slots mapped onto the recorded takes,
//      van photos, flash moments, contact card) and writes effectcraft/showcase.plan.json;
//   2. validates that the van photos / take frames are actually on disk, so EffectCraft
//      never imports into a half-ready project;
//   3. when EFFECTCRAFT_CLI is configured, runs the ExtendScript build-showcase.js head-less
//      — otherwise prints the exact recipe for running it inside EffectCraft (the same way
//      film/effectcraft/build-film.js is run today), and waits for the export afterwards.
//
// Export: H.264 MP4 master -> output/showcase-master.mp4 (EFFECTCRAFT_EXPORT_PATH).
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import { config, checkVanPhotos, logger } from '../config.js';
import { buildPlan } from './plan.js';

const PLAN_FILE = path.join(config.effectcraftDir, 'showcase.plan.json');

/** Every take it expects, matched to the recorder scripts' names. */
export function missingInputs(): string[] {
  const plan = buildPlan();
  const missing: string[] = [];
  for (const scene of plan.scenes) {
    const takeName = scene.take?.split(/[\\/]/).pop();
    const first = takeName
      ? path.join(config.root, ['videos', 'web', takeName, 'seq', `${takeName}_00000.jpg`].join('/'))
      : null;
    const firstMobile = takeName
      ? path.join(config.root, ['videos', 'mobile', takeName, 'seq', `${takeName}_00000.jpg`].join('/'))
      : null;
    if (takeName && !fs.existsSync(first!) && !fs.existsSync(firstMobile!)) {
      missing.push(`${first!} (or the tablet twin)`);
    }
  }
  missing.push(...checkVanPhotos());
  return missing;
}

async function waitForExport(timeoutMs = 30 * 60_000): Promise<boolean> {
  const out = config.effectcraft.exportPath;
  const deadline = Date.now() + timeoutMs;
  logger.info({ out }, 'waiting for the EffectCraft export…');
  while (Date.now() < deadline) {
    if (fs.existsSync(out)) {
      logger.info({ out }, 'master found');
      return true;
    }
    await new Promise((r) => setTimeout(r, 5000));
  }
  logger.warn({ out }, 'no export appeared within the timeout');
  return false;
}

async function runCli(scriptPath: string): Promise<boolean> {
  const cli = config.effectcraft.cli;
  if (!cli) return false;
  logger.info({ cli }, 'running the EffectCraft build head-less');
  await new Promise<void>((resolve, reject) => {
    const child = spawn(cli, [scriptPath], { shell: true, stdio: 'inherit' });
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`EffectCraft CLI exited ${code}`))));
    child.on('error', (err: Error) => reject(err));
  });
  return true;
}

export async function assemble(): Promise<{ master: string | null }> {
  const plan = buildPlan();

  // Fail fast on unusable inputs, with the exact fix for each.
  const bad = [
    ...checkVanPhotos().map((p) => `van photo missing: ${p} (check VAN_PHOTOS_ROOT)`),
    ...missingInputs().filter((m) => m.includes('(or the tablet twin)')),
  ];
  if (bad.length) {
    logger.error({ missing: bad }, `${bad.length} input(s) unusable`);
    throw new Error(`Inputs missing:\n${bad.join('\n')}`);
  }

  logger.info(
    {
      scenes: plan.scenes.length,
      recorded: plan.scenes.filter((s) => s.take).length,
      flashes: plan.scenes.reduce((n, s) => n + (s.flashAt?.length ?? 0), 0),
      audio: plan.audio ?? 'silent',
    },
    'writing the plan',
  );
  fs.writeFileSync(PLAN_FILE, JSON.stringify(plan, null, 1));

  const scriptPath = path.join(config.effectcraftDir, 'build-showcase.js');
  const ranHeadless = await runCli(scriptPath);

  if (!ranHeadless) {
      const recipeText = [
    `EffectCraft CLI not set — run the build by hand once:`,
    `  1) open the EffectCraft project at film-showcase/effectcraft/${config.effectcraft.projectName}.ecproj`,
    `  2) run:  var f = new File(app.project.file.parent.fsName + "/build-showcase.js");`,
    `           f.encoding = "UTF-8"; f.open("r"); eval(f.read()); f.close();`,
    `  3) export the "showcase film" comp to ${config.effectcraft.exportPath} (H.264 / MP4)`,
  ].join('\n');
  logger.warn({ recipe: recipeText }, 'manual step');
    await waitForExport();
  }

  const masterPath = config.effectcraft.exportPath;
  return { master: fs.existsSync(masterPath) ? masterPath : null };
}

// <name>_00000.jpg helpers live in missingInputs() above.

if (process.argv[1]?.replace(/\\/g, '/').endsWith('assemble.ts')) {
  assemble()
    .then(({ master }) => logger.info({ master }, 'assemble complete'))
    .catch((err) => {
      logger.error({ err: (err as Error).message }, 'assemble failed');
      process.exitCode = 1;
    });
}
