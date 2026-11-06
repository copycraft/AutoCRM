// Builds the 2-minute "Autotherm: A Teljes Folyamat" showcase from showcase.plan.json,
// written by film-showcase/src/effectcraft/assemble.ts: a comp per scene and the 120 s
// master "showcase film" with push-through transitions and a marker at each scene, in the
// look of film/motion.md (the app's own palette, IBM Plex Sans / Segoe UI, section labels,
// captions, browser and tablet mockups).
//
// Run from EffectCraft on a project whose .ecproj sits in film-showcase/effectcraft/:
//
//   var f = new File(app.project.file.parent.fsName + "/build-showcase.js");
//   f.encoding = "UTF-8"; f.open("r"); eval(f.read()); f.close();
//
// Footage is imported on demand: JPEG sequences from film-showcase/videos/{web,mobile} and
// the van photos from the plan's "photos" (the autotherm-photos root). A re-record rebuilds
// the master: run this script again, then export to film-showcase/output/showcase-master.mp4.

(function () {
  var lib = new File(app.project.file.parent.fsName + "/../../film/effectcraft/lib.js");
  lib.encoding = "UTF-8";
  lib.open("r");
  eval(lib.read());
  lib.close();

  var plan = readJson("showcase.plan.json");
  var W = plan.width || 1920;
  var H = plan.height || 1080;
  var FPS = plan.fps || 30;
  var PRE = 0.2; // a scene starts this much before its slot, under the previous one's exit
  var POST = 0.25; // and runs this much past it
  var MARGIN_X = 154; // the browser window's left edge: labels line up with it
  var MARGIN_Y = 35;

  app.beginUndoGroup("build showcase");

  // A rebuild drops its old comps; their unused solids are cleared at the end.
  for (var i = app.project.numItems; i >= 1; i--) {
    var it0 = app.project.item(i);
    if (it0 instanceof CompItem && (it0.name === "showcase film" || it0.name.indexOf("show ") === 0)) it0.remove();
  }

  // --- footage -------------------------------------------------------------

  var projectDir = app.project.file.parent.fsName;

  // Find in the project, or import, the footage at an absolute path.
  function importFootage(fpath, asSequence) {
    for (var i = 1; i <= app.project.numItems; i++) {
      var item = app.project.item(i);
      if (item instanceof FootageItem && item.mainSource && item.mainSource.file &&
          item.mainSource.file.fsName === fpath) return item;
    }
    var file = new File(fpath);
    var io = new ImportOptions(file);
    if (asSequence) {
      io.sequence = true; // the numbered JPEG frames import as one editable sequence
    }
    return app.project.importFile(io);
  }

  // A scene's recording: the take's first frame imports the whole JPEG sequence.
  function takeFootage(take) {
    var name = take.split("/").pop();
    var first = projectDir + "/../" + take + "/" + name + "_00000.jpg";
    var f = new File(first);
    if (!f.exists) throw new Error("take frames missing: " + f.fsName);
    var item = importFootage(first, true);
    if (item.mainSource instanceof FileSource) {
      item.mainSource.conformFrameRate = FPS; // the sequence plays at the capture fps
    }
    return item;
  }

  function photoFootage(key) {
    var fpath = plan.photos[key];
    if (!fpath) throw new Error("no photo for " + key);
    if (!new File(fpath).exists) throw new Error("photo missing: " + fpath);
    return importFootage(fpath, false);
  }

  // --- building blocks -----------------------------------------------------

  function solid(comp, color, name) {
    return comp.layers.addSolid(color, name, W, H, 1);
  }

  // The recording inside a browser window (like build-take.js's look): a bar with three
  // dots and an AutoCRM address pill, a soft shadow, the recording rounded into the body.
  // Returns the layers so a scene can hand the window to its own transitions.
  function browserMockup(comp, footage, x, y, w, t0, t1) {
    var h = w * (footage.height / footage.width);
    var bar = Math.max(22, w * 0.03);

    var win = shapeLayer(comp, "window");
    var body = group(win, "body");
    rect(body, [x + w / 2, y + (h + bar) / 2], [w, h + bar], 14);
    fill(body, BAR);
    var dots = group(win, "dots");
    for (var d = 0; d < 3; d++) ellipse(dots, [x + 24 + d * 20, y + bar / 2], [12, 12]);
    fill(dots, STEEL_500, 45);
    var pill = group(win, "address");
    rect(pill, [x + w / 2, y + bar / 2], [300, 20], 10);
    stroke(pill, STEEL_200, 1);
    fill(pill, [1, 1, 1]);
    shadow(win, 35, 24, 60);

    var address = text(comp, "AutoCRM", 13, STEEL_500, "Regular", ParagraphJustification.CENTER_JUSTIFY);
    prop(address, "ADBE Position").setValue([x + w / 2, y + bar / 2 + 4]);

    var screen = comp.layers.add(footage);
    screen.name = "screen";
    var fs = (w / footage.width) * 100;
    prop(screen, "ADBE Scale").setValue([fs, fs]);
    prop(screen, "ADBE Position").setValue([x + w / 2, y + bar + h / 2]);
    roundedMask(screen, footage.width, footage.height, 14 / (fs / 100));
    screen.inPoint = t0;
    screen.outPoint = t1;
    return { win: win, address: address, screen: screen };
  }

  // The recording inside a tablet frame: rounded dark body, 14-px-bezel-like ring, a
  // punch-hole camera, the recording masked to the screen's corners.
  function tabletMockup(comp, footage, cx, cy, w, t0, t1) {
    var screenW = w * 0.9;
    var screenH = screenW * (footage.height / footage.width);
    var bezel = Math.max(14, screenW * 0.04);
    var bodyW = screenW + bezel * 2;
    var bodyH = screenH + bezel * 2;

    var body = shapeLayer(comp, "tablet body");
    var bc = group(body, "body");
    rect(bc, [cx, cy], [bodyW, bodyH], 28);
    fill(bc, [0.06, 0.08, 0.09]);
    stroke(bc, STEEL_500, 1);
    shadow(body, 40, 26, 70);
    var cam = group(body, "camera");
    ellipse(cam, [cx, cy - bodyH / 2 + bezel / 1.6], [9, 9]);
    fill(cam, [0.14, 0.16, 0.17]);

    var screen = comp.layers.add(footage);
    screen.name = "tablet screen";
    var fs = (screenW / footage.width) * 100;
    prop(screen, "ADBE Scale").setValue([fs, fs]);
    prop(screen, "ADBE Position").setValue([cx, cy]);
    roundedMask(screen, footage.width, footage.height, 12 / (fs / 100));
    screen.inPoint = t0;
    screen.outPoint = t1;
    return { body: body, screen: screen };
  }

  // The drawn email approval (scene 3's right half): a card with From, Subject and lines.
  function emailCard(comp, x, y, w, lines, t0, t1) {
    var pad = 28;
    var lineH = 42;
    var h = pad * 2 + 96 + Math.max(0, lines.length - 1) * lineH;
    var card = shapeLayer(comp, "email card");
    var c = group(card, "card");
    rect(c, [x + w / 2, y + h / 2], [w, h], 16);
    fill(c, PANEL);
    stroke(c, STEEL_200, 1.5);
    shadow(card, 30, 18, 50);

    var from = text(comp, "Szabó Gergely <szabo.gergely@hopehely.example>", 19, STEEL_500);
    prop(from, "ADBE Position").setValue([x + pad, y + pad]);
    var subject = text(comp, lines[0] || "", 24, STEEL_700, "Semibold");
    prop(subject, "ADBE Position").setValue([x + pad, y + pad + 42]);
    var divider = shapeLayer(comp, "email divider");
    var g = group(divider, "rule");
    rect(g, [x + w / 2, y + pad + 84], [w - pad * 2, 2], 0);
    fill(g, BAR);
    var layers = [card, from, subject, divider];
    for (var i = 1; i < lines.length; i++) {
      var t = text(comp, lines[i], 25, STEEL_700);
      prop(t, "ADBE Position").setValue([x + pad, y + pad + 110 + (i - 1) * lineH]);
      layers.push(t);
    }
    for (var k = 0; k < layers.length; k++) span(layers[k], t0, t1, 0.4, 0.2);
    return card;
  }

  // A camera "flash" when a photo is taken in scene 4: a white blink on the screen and
  // a small ring beside the tablet, like a shutter.
  function photoFlash(comp, cx, cy, t) {
    var f = solid(comp, [1, 1, 1], "flash");
    var a = prop(f, "ADBE Opacity");
    a.setValuesAtTimes([t, t + 0.06, t + 0.14], [0, 60, 0]);
    f.inPoint = Math.max(0, t);
    f.outPoint = t + 0.16;

    var ring = shapeLayer(comp, "flash ring");
    var g = group(ring, "ring");
    ellipse(g, [0, 0], [70, 70]);
    stroke(g, [1, 1, 1], 4);
    prop(ring, "ADBE Position").setValue([cx, cy]);
    var rs = prop(ring, "ADBE Scale");
    rs.setValuesAtTimes([t, t + 0.35], [[10, 10], [190, 190]]);
    var ro = prop(ring, "ADBE Opacity");
    ro.setValuesAtTimes([t, t + 0.35], [50, 0]);
    ring.inPoint = t;
    ring.outPoint = t + 0.4;
  }

  // The film title over the opening photo: big line, subtle blur-settle, orange rule.
  function titleCard(comp, big, small, at) {
    var a = text(comp, big, 92, PANEL, "Semibold", ParagraphJustification.CENTER_JUSTIFY);
    prop(a, "ADBE Position").setValue([W / 2, H / 2 - 14]);
    prop(a, "ADBE Opacity").setValuesAtTimes([at, at + 0.9], [0, 100]);
    var sc = prop(a, "ADBE Scale");
    sc.setValuesAtTimes([at, at + 0.9], [[106, 106], [100, 100]]);
    ease(sc, 75);
    blur(a).setValuesAtTimes([at, at + 0.9], [8, 0]);
    var b = text(comp, small, 36, PANEL, "Regular", ParagraphJustification.CENTER_JUSTIFY);
    prop(b, "ADBE Position").setValue([W / 2, H / 2 + 62]);
    prop(b, "ADBE Opacity").setValuesAtTimes([at + 0.4, at + 1.1], [0, 85]);
    var rule = shapeLayer(comp, "title rule");
    var g = group(rule, "rule");
    rect(g, [W / 2, H / 2 + 20], [64, 3], 0);
    fill(g, SIGNAL);
    prop(rule, "ADBE Opacity").setValuesAtTimes([at + 0.35, at + 0.95], [0, 100]);
  }

  // --- the scenes ----------------------------------------------------------

  var captions = {
    2: "Új érdeklődés a weboldalról, egyenesen a rendszerben",
    3: "Ajánlatlevél → a megrendelés egy kattintással",
    4: "Fotólista rendszám szerint · egy fotó sem vész el",
    5: "Kártya a műhely után halad · akadály határidővel",
    6: "A számla a megrendelésből készül",
    7: "Aláírás a kijelzőn, az átvételkor rögzített sérülésekkel együtt",
    8: "Jelentések forintban · az udvar hőtérképe",
  };

  var sceneComps = [];
  for (var s = 0; s < plan.scenes.length; s++) {
    var sc = plan.scenes[s];
    var length = sc.end - sc.start;
    var comp = app.project.items.addComp("show " + ("0" + sc.n).slice(-2) + " " + sc.name, W, H, 1, length + PRE + POST, FPS);
    comp.motionBlur = true;
    solid(comp, STEEL, "background");
    var t0 = PRE;
    var t1 = PRE + length + POST;

    if (sc.n === 1) {
      // Scene 1: the van photo on track 1 with a slow push, the film title fading in.
      var p = photoFootage("sprinter");
      var l = comp.layers.add(p);
      var cover = (W / p.width) * 100;
      prop(l, "ADBE Scale").setValuesAtTimes([t0, t1], [[cover, cover], [cover * 1.1, cover * 1.1]]);
      var pos = prop(l, "ADBE Position");
      pos.setValuesAtTimes([t0, t1], [[W / 2 - 20, H / 2 + 10], [W / 2, H / 2]]);
      l.inPoint = t0;
      l.outPoint = t1;
      prop(l, "ADBE Opacity").setValuesAtTimes([t0, t0 + 1.2], [0, 100]);
      var dim = solid(comp, [0, 0, 0], "dim");
      prop(dim, "ADBE Opacity").setValuesAtTimes([t0 + 2, t0 + 3], [0, 45]);
      titleCard(comp, plan.title, plan.subtitle, t0 + 2.4);
      sectionLabel(comp, sc.label, MARGIN_X, MARGIN_Y, t0 + 0.3, t0 + Math.min(3.4, length));
      caption(comp, captions[1] || "Nyitás", t0 + 0.6, t0 + 4);
      continue;
    }

    var footage = null;
    if (sc.take) footage = takeFootage(sc.take);
    if (!footage) {
      // No recording yet: the slate stands in (timecode + what goes here), like build-film.js.
      var slate = shapeLayer(comp, "slate");
      var sg = group(slate, "card");
      rect(sg, [W / 2, H / 2], [1400, 500], 18);
      stroke(sg, STEEL_500, 1);
      fill(sg, STEEL_700);
      var tag = text(comp, "FELVÉTEL KÉSZÜL — " + (sc.take || "nincs take"), 34, PANEL);
      prop(tag, "ADBE Position").setValue([W / 2 - 660, H / 2 - 60]);
      var tag2 = text(comp, sc.label, 24, SIGNAL, "Semibold");
      prop(tag2, "ADBE Position").setValue([W / 2 - 660, H / 2 + 40]);
      span(slate, t0, t1, 0.3, 0.2);
      span(tag, t0, t1, 0.3, 0.2);
      span(tag2, t0, t1, 0.3, 0.2);
      sceneComps.push(comp);
      continue;
    }

    var durationIn = Math.min(length + PRE + POST, footage.duration || length + PRE + POST);

    if (sc.kind === "split") {
      // Scene 3: split screen — the web quote on the left, the drawn email on the right.
      var halfW = W * 0.46;
      var left = browserMockup(comp, footage, MARGIN_X, MARGIN_Y * 2, halfW, t0, t0 + durationIn);
      var right = emailCard(comp, W * 0.5 + 40, MARGIN_Y * 2 + 60, W * 0.46 - 80, sc.emailLines || [], t0 + 1.6, t1 - 0.8);
      caption(comp, captions[sc.n] || "", t0 + 1.4, t0 + 6.5);
      sectionLabel(comp, sc.label, MARGIN_X, MARGIN_Y, t0 + 0.3, t0 + Math.min(3.4, length));
    } else if (sc.kind === "tablet") {
      // Scenes 4 and 7: the tablet recording in the device frame, photo flashes on shots.
      var tab = tabletMockup(comp, footage, W / 2, H / 2 + 10, H * 0.9, t0, t0 + durationIn);
      var flashes = sc.flashAt || [];
      for (var fi = 0; fi < flashes.length; fi++) {
        photoFlash(comp, W / 2 + W * 0.22, H / 2 - H * 0.2, t0 + Math.min(1 + fi * 2.4, length - 0.5));
      }
      caption(comp, captions[sc.n] || "", t0 + 1.4, t0 + 6.5);
      sectionLabel(comp, sc.label, MARGIN_X, MARGIN_Y, t0 + 0.3, t0 + Math.min(3.4, length));
    } else {
      // Web scenes 2, 5, 6, 8: the browser mockup at 84 % width.
      var w4 = W * 0.84;
      var win3 = browserMockup(comp, footage, (W - w4) / 2, MARGIN_Y * 2.1, w4, t0, t0 + durationIn);
      caption(comp, captions[sc.n] || "", t0 + 1.4, t0 + 6.5);
      sectionLabel(comp, sc.label, MARGIN_X, MARGIN_Y, t0 + 0.3, t0 + Math.min(3.4, length));

      if (sc.n === 8) {
        // Outro: fade the reports, fade in the Autotherm logo and contact info.
        var f = plan.duration - sc.start; // scene length
        var dimOut = solid(comp, [0, 0, 0], "dim");
        prop(dimOut, "ADBE Opacity").setValuesAtTimes([t0 + f - 9, t0 + f - 7.5], [0, 62]);
        var brand = text(comp, plan.contact.line1, 100, PANEL, "Semibold", ParagraphJustification.CENTER_JUSTIFY);
        prop(brand, "ADBE Position").setValue([W / 2, H / 2 - 40]);
        prop(brand, "ADBE Opacity").setValuesAtTimes([t0 + f - 8, t0 + f - 6.8], [0, 100]);
        blur(brand).setValuesAtTimes([t0 + f - 8, t0 + f - 7], [12, 0]);
        var contact = text(comp, plan.contact.line2 + "  ·  " + plan.contact.line3, 38, PANEL, "Regular", ParagraphJustification.CENTER_JUSTIFY);
        prop(contact, "ADBE Position").setValue([W / 2, H / 2 + 56]);
        prop(contact, "ADBE Opacity").setValuesAtTimes([t0 + f - 7.4, t0 + f - 6.2], [0, 85]);
        prop(dimOut, "ADBE Opacity").setValuesAtTimes([t0 + f - 1.5, t0 + f + PRE], [62, 100]);
        var black = solid(comp, [0, 0, 0], "fade to black");
        prop(black, "ADBE Opacity").setValuesAtTimes([t0 + f - 1.5, t0 + f + 0.2], [0, 100]);
      }
    }

    sceneComps.push(comp);
  }

  // --- the master ----------------------------------------------------------

  var film = app.project.items.addComp("showcase film", W, H, 1, plan.duration, FPS);
  film.motionBlur = true;
  solid(film, STEEL, "background");
  for (var k2 = 0; k2 < plan.scenes.length; k2++) {
    var sc3 = plan.scenes[k2];
    var layer = film.layers.add(sceneComps[k2]);
    layer.startTime = sc3.start - PRE;
    layer.inPoint = Math.max(0, sc3.start - PRE);
    layer.outPoint = Math.min(plan.duration, sc3.end + POST);
    var times = [];
    var opacity = [];
    var scale = [];
    var blurs = [];
    if (k2 > 0) {
      times.push(sc3.start - PRE, sc3.start - PRE + 0.5);
      opacity.push(0, 100);
      scale.push([94, 94], [100, 100]);
      blurs.push(14, 0);
    }
    if (k2 < plan.scenes.length - 1) {
      times.push(sc3.end + POST - 0.45, sc3.end + POST);
      opacity.push(100, 0);
      scale.push([100, 100], [106, 106]);
      blurs.push(0, 14);
    }
    if (times.length) {
      prop(layer, "ADBE Opacity").setValuesAtTimes(times, opacity);
      prop(layer, "ADBE Scale").setValuesAtTimes(times, scale);
      ease(prop(layer, "ADBE Scale"), 60);
      blur(layer).setValuesAtTimes(times, blurs);
    }
    try {
      film.markerProperty.setValueAtTime(sc3.start, new MarkerValue(sc3.n + " · " + sc3.name));
    } catch (e) {}
  }

  // Music bed: the plan names the audio file when AUDIO_BED_PATH was supplied. With none
  // the master simply renders silent — the timeline structure stays valid either way.
  if (plan.audio) {
    try {
      var bed = film.layers.add(importFootage(plan.audio, false));
      bed.name = "music";
      bed.moveToEnd();
      var downAt = plan.scenes[1] ? plan.scenes[1].start : 10;
      bed.property("ADBE Audio Group").property("ADBE Audio Levels").setValuesAtTimes(
        [downAt, downAt + 1.5, plan.duration - 12, plan.duration - 10],
        [[0, 0], [-8, -8], [-8, -8], [0, 0]]
      );
    } catch (e) {
      // A missing audio asset is not fatal: the master renders silent.
    }
  }

  removeUnusedSolids();
  film.openInViewer();
  app.endUndoGroup();
  return "showcase film: " + plan.scenes.length + " scenes, " + film.duration + " s — export to film-showcase/output/showcase-master.mp4";
})();
