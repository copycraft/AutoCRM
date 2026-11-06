// A smoke test for the capture engine that does NOT need the dev servers: it drives
// take() against a tiny in-browser page (route-fulfilled), and prints the artifacts.
//   node_modules/.bin/tsx .smoke/capture-smoke.ts
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';

import { config, logger } from '../src/config.js';
import { take } from '../src/playwright/capture.js';

// A fake "app" to record: a 1920x1080 dark panel with a button and a canvas.
const server = http.createServer((req, res) => {
  res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
  res.end(`<html><body style="margin:0;background:#1B2327">
    <a href="/hu/links" style="position:absolute;left:200px;top:90px;color:#F7F8F7">Érdeklődés</a>
    <canvas id="sign" style="position:absolute;left:860px;top:300px;width:400px;height:200px;background:#F7F8F7"></canvas>
    <div id="data" style="position:absolute;left:200px;top:500px;color:#F7F8F7;font:28px Segoe UI">Jelentés: 0 Ft
      <script>setInterval(()=>{document.querySelector('#data').innerText='Jelentés: '+Math.floor(Date.now()/1000)+' Ft'},500)</script></div>
  </body></html>`);
});

config.webUrl = 'http://127.0.0.1:8642';
config.mobileUrl = 'http://127.0.0.1:8642';

server.listen(8642, async () => {
  logger.info('smoke serve on 8642');
  try {
    await take('smoke-desktop', { seed: 7, start: '/', skipLogin: true }, async (rec) => {
      await rec.wait(600);
      await rec.click(rec.page.locator('a'));
      await rec.wait(600);
      await rec.type('Sprinter');
      await rec.scroll(120, 500);
      await rec.sign(rec.page.locator('#sign'), [
        [[0.1, 0.7], [0.3, 0.3], [0.5, 0.6], [0.7, 0.25], [0.9, 0.65]],
      ]);
      await rec.beat('report', rec.page.locator('#data'), 800);
    });
    await take('smoke-tablet', { seed: 7, start: '/', skipLogin: true, device: 'Galaxy Tab S4' }, async (rec) => {
      await rec.wait(500);
      await rec.click(rec.page.locator('a'));
      await rec.wait(400);
    });
    for (const name of ['smoke-desktop', 'smoke-tablet']) {
      const dir = name === 'smoke-tablet' ? config.videos.mobile : config.videos.web;
      const seq = path.join(dir, name, 'seq');
      const files = fs.readdirSync(seq);
      const track = JSON.parse(fs.readFileSync(path.join(dir, name, `${name}.track.json`), 'utf8')) as { duration: number; cursor: unknown[]; events: unknown[]; beats: unknown[] };
      logger.info({ name, frames: files.length, duration: track.duration.toFixed(2), cursor: track.cursor.length, events: track.events.length, beats: track.beats.length }, 'smoke artifact check');
    }
  } finally {
    server.close();
  }
});
