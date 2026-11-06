# AutoCRM film: motion plan

How the film moves, and how the camera knows where to look. Goes with [script.md](script.md):
the script says *what* is on screen; this says how it moves.

## The idea in one line

Every screen recording sits under a **virtual camera** that follows the cursor (or the
finger on the phone) and pushes in on whatever the narration is talking about. The cursor
and every move come from data logged while recording, not from hand-set keyframes, so each
take re-cuts itself when it is re-recorded.

## Rules

| | |
|---|---|
| **Move only when attention moves.** | The camera is still while something is being read or typed. |
| **One ease for the camera.** | 0.7 s, ease-in-out (bezier influence 75 % out / 75 % in), no overshoot. |
| **UI pops may overshoot.** | Cards, toasts, badges: 0.35 s, 3 % overshoot, settle. |
| **Hold after every move.** | At least 1.2 s before the next one, so the eye can land. |
| **Zoom range.** | Web 1.0–2.0×, phone 1.0–1.6×. Over 2× the UI stops reading as an app. |
| **Motion blur** | On for the camera and the cursor (180° shutter), off for text. |
| **Nothing flashy** | No light leaks, glitches or whip pans. This is a working tool for a workshop. |

## Look

- **Colours from the app** (`frontend/tailwind.config.ts`): background `steel-900 #1B2327` with a
  very slow, faint gradient drift; accent `signal #E8590C` for highlights and the cursor
  ring; `cold #0F5C7A` for the stage rail; `done #2F7A3E` for "Kész" and green ticks; text
  `panel #F7F8F7`.
- **Font: IBM Plex Sans**, the app's own font. It isn't installed on this PC (OFL, free from
  Google Fonts); until it is, Segoe UI.
- **Browser frame:** a 14 px rounded window, a thin top bar with three dots and an "AutoCRM"
  address pill (no real hostname), a soft shadow (blur 60, 35 %). At rest it fills 84 % of
  the frame width, which leaves the top margin for the section label; the camera zooms the
  window with its frame, like a real camera.
- **Phone frame:** drawn as shapes (rounded body, 14 px bezel, punch-hole camera), a 3D layer
  so it can tilt. The recording is masked to the screen's corner radius.
- **Captions:** bottom-centred, Plex Sans Medium 34 px on a 70 % `steel-900` pill; rise 12 px
  and fade in over 0.3 s, fade out over 0.25 s.
- **Section labels** (`05 · ÁTVÉTEL`): top-left in the margin above the window, Plex Sans
  SemiBold 22 px, letter spacing +12 %, an orange 3 px bar before the number. Slide in 24 px
  from the left, characters staggered 15 ms; they stay 2.5 s and fade, since the first
  push-in covers the margin.
- **Key chips:** a pressed shortcut ("/", Esc) shows as a dark keycap at the bottom centre
  for 0.8 s.

## Mouse tracking

### 1. While recording (web)

The web takes are Playwright scripts. Nothing is clicked by hand, so every take can be redone.

- **The capture is our own**, not Playwright's video (VP8 at 1 Mbit/s, too soft to zoom
  into). Chrome's screencast (CDP `Page.startScreencast`) sends a frame on every repaint, at
  **2× device scale (3840×2160)** so a 2× zoom still lands as sharp 1080p. Frames are saved
  with their timestamps and turned into a steady 30 fps PNG sequence (each frame held until
  the next repaint). EffectCraft imports PNG sequences directly.
- **The cursor is moved like a hand:** each move is a curve (slight arc, ease-in-out,
  250–700 ms depending on distance, a 2–4 px overshoot that settles), never a straight jump.
  Typing runs at 60–110 ms per key with small random gaps.
- **The track file** (`<take>.track.json`), written next to the frames:

  ```json
  {
    "fps": 30, "scale": 2,
    "cursor":  [{"t": 1.233, "x": 812, "y": 440}],
    "events":  [{"t": 2.010, "type": "down", "x": 1290, "y": 618}, {"t": 2.090, "type": "up"},
                {"t": 3.400, "type": "type", "until": 5.100}, {"t": 6.000, "type": "navigate"}],
    "beats":   [{"t": 4.0, "until": 7.5, "name": "quote-value", "rect": [1180, 560, 420, 96]}]
  }
  ```

  `beats` are the important part: the recording script names the element the narration
  talks about (`beat("quote-value", "#quote-value")`), and its on-screen rectangle at that
  moment is logged. The camera and spotlights come from these rectangles, so they are exact.

### 2. While recording (phone)

- `adb shell screenrecord` on the phone (or the emulator), 1080 px wide, H.264.
- The taps are driven by a script (`adb shell input tap/swipe`), so their positions and times
  are known and logged into the same kind of track file. Scenes that need a real hand (the
  camera shots on the real phone) use `adb shell getevent` to log the touches instead.
- Status bar cleaned with Android's demo mode: full battery, full signal, the clock at 9:41.

### 3. The camera (from track file to keyframes)

A Node script (`film/motion/camera.mjs`) turns a track file into keyframes for a null layer
that the recording is parented to:

1. **In a beat:** zoom so the beat's rectangle fills about 55 % of the frame (within the
   zoom range), centred on it. The move starts 0.3 s before the beat, so the camera arrives
   as the narration names the thing.
2. **Between beats:** follow the cursor with a dead zone. The camera stays put while the
   cursor is inside the middle 60 % of the frame, and moves only when it leaves it. The
   movement is a critically damped spring (about 0.5 s to settle), so it never wobbles.
3. **Zoom out** to 1.0× on every page change, and whenever a beat's rectangle is wider than
   the frame allows.
4. **Never show past the edge:** the camera is clamped so the frame never leaves the page.
5. **Thin the keys:** one value per frame, then simplified (Ramer–Douglas–Peucker, 0.5 px)
   to a few keys per second, with eases kept.

EffectCraft's scripting then applies them (`setValuesAtTimes` on position and scale), turns
on motion blur for the layer, and saves everything into the project.

### 4. The cursor and taps

- **The cursor is a layer, not part of the recording:** a white macOS-style arrow at 1.3× with
  a thin dark outline and a soft shadow. Its position comes from the track (smoothed with a
  one-euro filter so it doesn't jitter). It fades out after 1.5 s without movement, and
  fades back in when it moves again.
- **Click:** the arrow dips to 85 % and back (0.12 s), and an orange ring grows from 0 to
  60 px while fading from 60 % to 0 % (0.4 s).
- **Typing:** a faint caret glow on the focused field (from its beat rectangle); no
  typewriter sound.
- **Phone taps:** a white 30 % disc (44 px) that shrinks and fades in 0.35 s; a swipe
  leaves a short fading trail.

### 5. Spotlight

For the few moments that need it (the NAV status, the VIES check, the stage gate), a
`steel-900` overlay at 55 % with a rounded-rectangle hole cut at the beat's rectangle
(feather 24 px), plus a 2 px orange outline that draws itself around the hole in 0.5 s.
In 0.4 s, out 0.3 s. At most one in each scene.

## Transitions

- **Between scenes:** "push through". The outgoing scene scales 100→106 %, blurs 0→14 and
  fades over 0.45 s; the incoming one comes from 94 % and blurring 14→0 over 0.5 s,
  overlapping by 0.2 s. The section label lands 0.2 s after.
- **Web ↔ phone in the same scene:** the browser slides 18 % to the left and shrinks to
  82 %, while the phone rises in from below on the right, tilted −12° in Y, and settles to
  −6° (a resting tilt that reads as 3D without being a gimmick).
- **Phone alone:** centred, tilt 0°, with a slow 2° drift in Y over the shot so it never
  sits dead still.

## Scene beats

Times are scene-relative. The camera beats are the `beat()` names the recording scripts use.

| # | Scene | Motion |
|---|---|---|
| 0 | Nyitás | Ken Burns on the three photos (100→110 % over 5 s, a different drift on each, 1 s crossfades). Title: letters track in (spacing +40 %→0, blur 8→0, 0.9 s), subtitle 0.4 s later. The photo behind darkens to 55 % and blurs to 18. |
| 1 | Az érdeklődés | Website form at 1.4× while typing (`enquiry-form`); click ripple on *Küldés*. Push to the CRM: spotlight on the bell, the bell swings ±12° three times. The notification card pops. The phone rises in on the right, the system banner drops in, tap ripple. Back to 1.0× for the sources chart, then a 1.3× push-in onto the bars. |
| 2 | Ajánlat | Camera follows the stage chips (`lead-stage`). The quote value counts up 0 → 6 850 000 Ft in the caption while the field is typed. The PDF slides up over the page as its own layer (shadow, 0.6 s), then back. Push-in on *Lejáró árajánlatok*. |
| 3 | Megrendelés | Convert dialog pops; the line items get a 1.2× pan from top to bottom. Build spec at 1.5× (`build-spec`). Stage rail: each stage lights `cold` blue in turn (0.25 s apart) with a small tick; "Kész" lights green last. |
| 4 | A telefon | Phone alone. The icon tap ripple, the fingerprint glyph pulses once. The drawer slides out; the camera pans down its list at 1.3×. Airplane mode: a small "nincs térerő" chip drops in beside the phone while the list stays. The theme switch is three quick crossfades (0.2 s). |
| 5 | Átvétel | Phone, tilted −6°. The camera pushes to 1.4× on the photo checklist; each tick gets a small green pop. Shutter: a 2-frame white flash on the phone screen only. The car outline at 1.6× (`car-outline`); the damage marker drops with a ring. The dictation waveform is a live-looking bar animation in a caption chip. Signatures: 1.5× on the pad; the stroke is the real recording. Then the browser slides in from the left showing the same inspection, the phone steps back. |
| 6 | Fotózás | A split: the phone on the left, the *Sor* list on the right as a large callout. "3 vár feltöltésre" with a no-signal icon; signal back, the three rows tick green one by one (0.3 s apart). |
| 7 | Gyártás | Web follow-cam. The blocker card pops; the due date gets an orange underline that draws on. The barcode scan: the phone at 1.6× with a scan line sweeping the viewfinder (from the real camera feed). The voice note: a small waveform chip flies from the phone to the browser's documents list (0.6 s arc). The stage gate: push-in on the list, "Kész (feltétel nem teljesül)" greyed out. |
| 8 | Udvar és kijelző | 2D yard at 1.0×, then the drag followed at 1.4× (`yard-card`). The switch to 3D is the app's own; the camera adds a slow push. The TV: the board placed into a simple wall-mounted TV frame, a 3° perspective, faint screen reflection. |
| 9 | Számlázás | The invoice preview slides up like the PDF in scene 2. Spotlight on the NAV status as it turns accepted. The list tabs: a quick 1.2× pan left to right across them. VIES: spotlight, a green tick pops beside the result. Phone scan: page thumbnails collect into a stack, then fold into one PDF icon (0.5 s). |
| 10 | Levelezés | The correspondence list at 1.3×; the customer's reply row gets a soft orange left bar. Newsletter stats: the open and click numbers count up. The website-conversion funnel: 1.2× push. |
| 11 | HR | Directory at 1.0×; push-in on the expiring-document flag (`hr-expiry`), which pulses once. Calendar: a slow pan across. The job page and the phone side by side. |
| 12 | Áttekintés | The dashboard tiles rise in one by one (0.08 s stagger) as an overlay copy, then hand over to the live page. Search: the "/" key shows as a small keycap chip. The assistant's answer is the real recording; the camera holds at 1.3× on it. |
| 13 | Kiadás és zárás | Phone: the damage-comparison card at 1.5×, the "már átvételkor is megvolt" tag pops green. The customer's signature, then the web's stage rail lights "Kész" green, and the TV board's card slides into *Átvehető*. Back to the van photo; end card as the title, music up, fade to black over 1.5 s. |

## Sound

- **Narration** at −16 LUFS, from `film/audio/vo/<scene>.wav`; each scene's picture is cut to
  its sentence timings (`<scene>.json` gives the lengths).
- **Music bed** at −28 LUFS, dipped by 8 dB under the narration (the dips keyed from the
  sentence timings, 0.3 s ramps), up to −20 LUFS for the opening and the end card. Track to
  be supplied.
- **UI sounds**, very quiet (−30 dB): a soft click on clicks and taps, a short tick on green
  ticks, a low whoosh on scene changes, a notification chime in scene 1. They need a CC0
  or licensed pack.

## Order of work

1. **A test take first:** 10 seconds of the dashboard captured at 2×, a track file, the
   camera script, imported into EffectCraft, rendered. This proves the pipeline end to end
   before anything long is recorded.
2. The data for filming (the film seed in script.md) and the phone setup.
3. Record the web takes scene by scene; then the phone takes.
4. Voice the narration, so the picture is cut to it.
5. Build the master comp: frames, cursor, camera, captions, labels, transitions, sound.
6. Render a 540p preview of the whole film to check the timing, then the 1080p master.
