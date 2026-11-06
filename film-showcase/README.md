# AutoCRM showcase film (film-showcase/)

The automated pipeline for the 2-minute **Autotherm: A Teljes Folyamat** film: Playwright
records the web and tablet takes, this workspace then drives the local EffectCraft
compositor to lay them over the real van photos, apply the motion-graphics mockups, and
export the H.264 master into `output/`.

Reuses the conventions proven in `../film/` (see `film/motion.md`): CDP screencast JPEG
sequences at 2× device scale (not Playwright's `recordVideo` webm — it is VP8 at 1 Mbit/s,
too soft to zoom into), camera data from track files, `film/effectcraft/lib.js` shape/text/
easing helpers.

## Pipeline

```
npm run film
  ├─ 1. clean            videos/ + output/
  ├─ 2. record-web.ts    s02 érdeklődés, s03 ajánlat, s05 gyártás, s06 számlázás, s08 reports
  ├─ 3. record-mobile.ts m04 átvétel, m07 kiadás  (Galaxy Tab S4 emulation, same web app)
  ├─ 4. assemble.ts      writes effectcraft/showcase.plan.json + build-showcase.js,
  │                        runs EFFECTCRAFT_CLI (or prints the manual recipe), waits for
  │                        the export
  └─ 5. output/showcase-master.mp4 (H.264 1920x1080, 30 fps, 120 s)
```

Each step is runnable alone: `npm run record:web`, `npm run record:mobile`, `npm run assemble`.

## Prerequisites

- Both dev servers (web + backend) running locally — default URLs in `.env`
  (`WEB_APP_URL=http://localhost:3000`, `BACKEND_URL=http://127.0.0.1:8080`).
- The film stack data (`film/capture/backend-film.ps1` + `film/capture/seed-film.mjs`), so
  the hero lead / MNP order exist; the takes look these up (`src/record-data.ts`).
- Real van photos at `VAN_PHOTOS_ROOT` (the `autotherm-photos` folder one level above the
  repo, next to the project directory).
- Playwright browsers: `npx playwright install chromium`.

## EffectCraft step

EffectCraft is the desktop After Effects-style compositor used by `../film/` (`.ecproj`
projects + ExtendScript). If `EFFECTCRAFT_CLI` cannot run its host head-less today, the
manual step is only the same node-sequence + comp-build + export dance build-film.js does:

1. Open EffectCraft with project `film-showcase/effectcraft/showcase-film.ecproj`
   (create one empty `showcase-film` project file there once — the pipeline never edits the
   old `../film/effectcraft/autocrm-film.ecproj`).
2. Run: `var f = new File(app.project.file.parent.fsName + "/build-showcase.js"); f.encoding = "UTF-8"; f.open("r"); eval(f.read()); f.close();`
3. Export the `showcase film` comp → `output/showcase-master.mp4` (H.264, 30 fps, 120 s).

`assemble.ts` then waits for the exported file (or completes immediately when a CLI is set).

## The storyboard the pipeline builds

| # | 0:00 | scene | on screen |
|---|-------|-------|-----------|
| 1 | 0:00–0:10 | Nyitás | real Sprinter photo + slow push-in, «Autotherm: A Teljes Folyamat» title fades in on a track-2 overlay |
| 2 | 0:10–0:20 | Érdeklődés | the new lead parsed on the web dashboard |
| 3 | 0:20–0:35 | Ajánlat & Megrendelés | split screen: web quote (browser mockup) | drawn email-approval card |
| 4 | 0:35–0:50 | Átvétel | tablet mockup, intake, camera "flash" when photos land |
| 5 | 0:50–1:15 | Gyártás | web Kanban moves + HR side-scene |
| 6 | 1:15–1:30 | Számlázás | the auto invoice |
| 7 | 1:30–1:45 | Kiadás | tablet mockup, the drawn digital signature |
| 8 | 1:45–2:00 | Reports & Outro | charts fade, Autotherm logo + contact fade in |

All on-screen text is Hungarian. Scene 1 and the outro are composed purely in EffectCraft
(photos + title), so no recording is spent on them.

## Notes

- Selectors that depend on the film seed (hero lead id, the MNP order, dialogs, canvases)
  are wrapped in `bestEffort()` — a re-watch logs `step skipped (timeout)` rather than
  crashing the take, so partial footage still lands and only the affected beat needs a
  re-record.
- The "digital signature" (scene 7) is drawn over a `<canvas>` when the handover page has
  one; the Android inspection pad is native-only, and its recording would land on this
  same `m07-kiadas` take (matching name), so a later phone-captured take replaces it without
  any pipeline change.
- Audio: `AUDIO_BED_PATH` in `.env`; with nothing supplied, the timeline keeps the audio
  track slot structure silent.
