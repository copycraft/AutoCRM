// The showcase film pipeline, end to end:
//   1. clean videos/ and output/
//   2. record-web.ts    — the five desktop takes (1920x1080 headless)
//   3. record-mobile.ts — the two tablet takes (Playwright device emulation)
//   4. assemble.ts      — writes effectcraft/showcase.plan.json + build-showcase.js and
//                         drives EffectCraft (CLI when configured, else the manual recipe)
//   5. the H.264 master lands in output/, the log reports where and how long it took.
//
//   npm run film     (or: tsx src/index.ts)
import fs from 'node:fs';
import path from 'node:path';

import { config, logger } from './config.js';
import { recordWeb } from './playwright/record-web.js';
import { recordMobile } from './playwright/record-mobile.js';
import { assemble } from './effectcraft/assemble.js';

function clean(): void {
  let cleared = 0;
  for (const dir of [config.videos.web, config.videos.mobile, config.output]) {
    if (fs.existsSync(dir)) {
      fs.rmSync(dir, { recursive: true, force: true });
      cleared++;
    }
    fs.mkdirSync(dir, { recursive: true });
  }
  logger.info({ cleared }, 'videos/ and output/ cleaned');
}

async function run(step: string, fn: () => Promise<void>): Promise<void> {
  const t0 = Date.now();
  logger.info({ step }, `${step} — start`);
  try {
    await fn();
  } catch (err) {
    logger.error({ step, err: (err as Error).message }, `${step} — failed`);
    throw err;
  }
  logger.info({ step, secs: ((Date.now() - t0) / 1000).toFixed(1) }, `${step} — done`);
}

async function main(): Promise<void> {
  const startedAt = Date.now();
  logger.info({ project: config.root }, 'AutoCRM showcase film pipeline');

  run('clean', async () => clean());
  run('record web', recordWeb);
  run('record tablet', recordMobile);
  const { master } = await assemble();

  const total = ((Date.now() - startedAt) / 1000).toFixed(1);
  if (master) {
    logger.info({ master, totalSecs: total }, `✓ master ready: ${path.resolve(master)} (${total} s total)`);
  } else {
    logger.warn(
      {
        totalSecs: total,
        output: path.resolve(config.effectcraft.exportPath),
      },
      `pipeline finished in ${total} s — recordings and the plan are ready; the master appears at output/showcase-master.mp4 once EffectCraft exports it (see the manual recipe above)`,
    );
  }
}

main().catch((err) => {
  logger.error({ err: (err as Error).message }, 'pipeline failed');
  process.exitCode = 1;
});
