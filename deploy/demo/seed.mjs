#!/usr/bin/env node
// Fills a fresh showcase with something to show: open, lost and won leads, orders at
// different stages (one waiting for an átvétel on the phone, one waiting for a kiadás),
// finished átvétel/kiadás records with damages and signatures, blockers, employees with
// days off and sick leave.
//
// It goes through the public API as the demo admin, so every record passes the same rules
// as one typed in by hand. Run it next to the demo's .env, with the stack and tunnel up:
//
//   node seed.mjs
//
// DEMO_PASSWORD and APP_PORT come from ./.env (or the environment); BASE_URL overrides the
// address (default http://127.0.0.1:APP_PORT). No node on the server?
//
//   docker run --rm --network host -v "$PWD:/d" -w /d node:20-alpine node seed.mjs
//
// Running it twice adds a second copy of everything; ./setup.sh starts clean.
//
// Photos are not seeded (a placeholder picture looks worse than none): the live átvétel on
// the phone is where the photography is shown.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

// ./.env, the one setup.sh reads: KEY=value lines; anything already in the environment wins.
try {
  for (const line of readFileSync(new URL('.env', import.meta.url), 'utf8').split('\n')) {
    const m = line.match(/^\s*([A-Z_][A-Z0-9_]*)\s*=\s*(.*?)\s*$/);
    if (m && !(m[1] in process.env)) process.env[m[1]] = m[2].replace(/^(["'])(.*)\1$/, '$2');
  }
} catch {
  // no .env beside the script: the environment has to carry it
}

const BASE = (process.env.BASE_URL ?? `http://127.0.0.1:${process.env.APP_PORT || 3000}`).replace(/\/$/, '');
const PASSWORD = process.env.DEMO_PASSWORD;
if (!PASSWORD) throw new Error('set DEMO_PASSWORD (in ./.env or the environment)');

let token = '';
const failures = [];

async function api(method, path, body) {
  const res = await fetch(`${BASE}/api${path}`, {
    method,
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    const e = new Error(`${method} ${path} -> ${res.status} ${data?.error?.code ?? ''} ${data?.error?.message ?? text}`);
    e.status = res.status;
    throw e;
  }
  return data;
}

/** One seeding step: a refusal is reported at the end, and the rest still runs. */
async function step(name, fn) {
  try {
    return await fn();
  } catch (e) {
    failures.push(`${name}: ${e.message}`);
    console.warn(`! ${name}: ${e.message}`);
    return undefined;
  }
}

const iso = (offsetDays) => {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() + offsetDays);
  return d.toISOString().slice(0, 10);
};
const huf = (forint) => forint * 100;

// ── A signature PNG: a white card with a looping stroke, no image library needed ──

function crc32(buf) {
  let c;
  const table = (crc32.table ??= Array.from({ length: 256 }, (_, n) => {
    c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  }));
  let crc = 0xffffffff;
  for (const byte of buf) crc = table[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function signaturePng(seed) {
  const w = 360;
  const h = 120;
  const px = Buffer.alloc(w * h, 255);
  const put = (x, y) => {
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const xx = Math.round(x) + dx;
        const yy = Math.round(y) + dy;
        if (xx >= 0 && xx < w && yy >= 0 && yy < h) px[yy * w + xx] = 30;
      }
    }
  };
  for (let t = 0; t < 1; t += 0.0008) {
    const x = 20 + t * (w - 40);
    const y = h / 2 + Math.sin(t * (14 + seed * 3)) * (22 + seed * 4) * (1 - t * 0.5) + Math.cos(t * 41) * 6;
    put(x, y);
  }
  const rows = Buffer.concat(
    Array.from({ length: h }, (_, y) => Buffer.concat([Buffer.from([0]), px.subarray(y * w, (y + 1) * w)])),
  );
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 0; // greyscale
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(rows)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** The three-step upload (ticket, PUT, complete) for a document of an order. */
async function uploadDocument(orderId, filename, bytes) {
  const ticket = await api('POST', `/orders/${orderId}/uploads`, {
    target: { type: 'document', kind: 'other' },
    filename,
    content_type: 'image/png',
    byte_size: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex'),
  });
  if (ticket.status === 'already_uploaded') return ticket.document_id;
  const put = await fetch(ticket.upload.url, {
    method: 'PUT',
    headers: Object.fromEntries(ticket.upload.headers),
    body: bytes,
  });
  if (!put.ok) throw new Error(`PUT to storage -> ${put.status}`);
  const done = await api('POST', '/uploads/complete', { ticket: ticket.ticket });
  return done.document.id;
}

// ── Records ──

async function seed() {
  const login = await api('POST', '/auth/login', {
    email: 'admin@demo.local',
    password: PASSWORD,
    client: 'mobile',
    device_label: 'demo seed',
  });
  token = login.token;

  const projectTypes = (await api('GET', '/project-types')).items.filter((t) => t.is_active);
  const lostReasons = (await api('GET', '/lost-reasons')).items.filter((r) => !r.archived_at);
  const sources = (await api('GET', '/lead-sources')).items.map((s) => s.key);
  const source = (i) => sources[i % Math.max(sources.length, 1)] ?? null;
  const type = (i) => projectTypes[i % projectTypes.length]?.id ?? null;

  // Partners and one contact each.
  const partnerSpecs = [
    ['business', 'Frigo-Trans Kft.', '24681357-2-41', 'Szabó Gergely', 'szabo.gergely@frigotrans.example', '+36 30 555 0101', 'Győr'],
    ['business', 'Bakery Express Zrt.', '13572468-2-13', 'Nagy Krisztina', 'nagy.krisztina@bakeryexpress.example', '+36 20 555 0102', 'Székesfehérvár'],
    ['business', 'Szent Mihály Temetkezési Kft.', '11223344-2-05', 'Tóth László', 'toth.laszlo@szentmihaly.example', '+36 70 555 0103', 'Pécs'],
    ['business', 'Alpen-Kühltransporte GmbH', null, 'Markus Huber', 'huber@alpen-kuehl.example', '+43 664 555 0104', 'Graz'],
    ['person', 'Kovács Péter', null, null, 'kovacs.peter@example.com', '+36 30 555 0105', 'Budapest'],
  ];
  const partners = [];
  for (const [kind, name, tax, contact, email, phone, city] of partnerSpecs) {
    const austrian = name.includes('GmbH');
    const p = await step(`partner ${name}`, () =>
      api('POST', '/partners', {
        kind,
        name,
        tax_number: austrian ? null : tax,
        country: austrian ? 'AT' : 'HU',
        default_currency: austrian ? 'EUR' : 'HUF',
        email,
        phone,
        city,
      }),
    );
    partners.push(p);
    if (p && contact) {
      await step(`contact ${contact}`, () => api('POST', `/partners/${p.id}/contacts`, { name: contact, email, phone, position: 'Flottafelelős' }));
    }
  }
  const [frigo, bakery, funeral, alpen, kovacs] = partners;

  // ── Leads: open (new, contacted, quoted), lost with reasons, won through conversion ──
  const lead = (title, partner, i, extra = {}) =>
    step(`lead ${title}`, () =>
      api('POST', '/leads', {
        title,
        partner_id: partner?.id ?? null,
        contact_name: partner ? undefined : 'Ismeretlen érdeklődő',
        source: source(i),
        description: 'Bemutató adat.',
        ...extra,
      }),
    );
  const moveLead = (l, stage, extra = {}) => l && step(`lead ${l.id} -> ${stage}`, () => api('POST', `/leads/${l.id}/stage`, { stage, ...extra }));

  // Open
  await lead('Hűtős felépítmény – 3 db Sprinter', frigo, 0, { quoted_value_minor: huf(18_900_000), currency: 'HUF', quote_valid_until: iso(14) }).then((l) => moveLead(l, 'quoted'));
  await lead('Pékáru-szállító furgon fűtés', bakery, 1).then((l) => moveLead(l, 'contacted', { note: 'Telefonon egyeztettünk.' }));
  await lead('Kegyeleti átalakítás – Crafter', funeral, 2).then(async (l) => {
    await moveLead(l, 'contacted');
    await moveLead(l, 'quoted');
  });
  await lead('Új érdeklődés a weboldalról', null, 3);

  // Lost
  const reason = (i) => lostReasons[i % Math.max(lostReasons.length, 1)]?.id;
  for (const [i, [title, partner]] of [
    ['Hűtőtér javítás – Ducato', kovacs],
    ['Egyedi hűtőkocsi – osztrák megrendelő', alpen],
    ['Raktér szigetelés – kistehergépkocsi', bakery],
  ].entries()) {
    const l = await lead(title, partner, i + 4, { quoted_value_minor: huf(2_400_000 + i * 700_000), currency: 'HUF' });
    await moveLead(l, 'contacted');
    await moveLead(l, 'quoted');
    await moveLead(l, 'lost', { lost_reason_id: reason(i), note: 'Másik ajánlatot fogadtak el.' });
  }

  // Won: converted to orders
  const orderSpecs = [
    { title: 'Hűtős felépítmény – Sprinter', partner: frigo, plate: 'MNP-421', make: 'Mercedes-Benz', model: 'Sprinter 316', items: [['Hűtőagregát beépítése', 1, 4_200_000], ['Szigetelt raktér', 1, 2_650_000]] },
    { title: 'Fűtött pékáru-szállító', partner: bakery, plate: 'RTK-883', make: 'Renault', model: 'Master', items: [['Fűtőrendszer', 1, 1_850_000], ['Polcrendszer', 2, 320_000]] },
    { title: 'Kegyeleti kocsi átalakítás', partner: funeral, plate: 'SZM-107', make: 'Volkswagen', model: 'Crafter', items: [['Kegyeleti belső kialakítás', 1, 5_400_000]] },
  ];
  const orders = [];
  for (const [i, spec] of orderSpecs.entries()) {
    const l = await lead(`${spec.title} (megnyert)`, spec.partner, i, { quoted_value_minor: huf(spec.items.reduce((s, [, q, p]) => s + q * p, 0)), currency: 'HUF' });
    if (!l) continue;
    await moveLead(l, 'contacted');
    await moveLead(l, 'quoted');
    const converted = await step(`convert ${spec.title}`, () =>
      api('POST', `/leads/${l.id}/convert`, {
        title: spec.title,
        partner_id: spec.partner.id,
        currency: 'HUF',
        project_type_id: type(i),
        vehicle_make: spec.make,
        vehicle_model: spec.model,
        vehicle_plate: spec.plate,
        due_date: iso(30 + i * 10),
        items: spec.items.map(([description, quantity, price]) => ({ description, quantity: String(quantity), unit_price: huf(price) })),
      }),
    );
    const order = converted?.order ?? converted;
    if (order?.id) orders.push({ ...order, spec, projectTypeId: type(i) });
  }

  // Two more orders made directly: one with no plate yet (the live átvétel on the phone shows
  // the plate being asked for and photographed), one for a private customer.
  const bare = await step('order without plate', () =>
    api('POST', '/orders', {
      partner_id: alpen.id,
      currency: 'EUR',
      title: 'Hűtős átalakítás – átvételre vár',
      project_type_id: type(0),
      vehicle_make: 'Iveco',
      vehicle_model: 'Daily',
      due_date: iso(45),
      items: [{ description: 'Hűtőtér', quantity: '1', unit_price: 920000 }],
    }),
  );
  await step('order private', () =>
    api('POST', '/orders', { partner_id: kovacs.id, currency: 'HUF', title: 'Hűtőtér javítás', project_type_id: type(1), vehicle_plate: 'KPE-552', vehicle_make: 'Fiat', vehicle_model: 'Ducato', items: [{ description: 'Javítás', quantity: '1', unit_price: huf(640_000) }] }),
  );

  // ── Stages and blockers ──
  const [first, second, third] = orders;
  for (const [o, mileage, stages] of [
    [first, 84_200, ['design', 'production']],
    [second, 121_500, ['design']],
    [third, 56_900, []],
  ]) {
    if (!o) continue;
    await step(`slip ${o.id}`, () => api('PATCH', `/orders/${o.id}`, { mileage_in: mileage, fuel_level: '1/2', key_count: 2, intake_condition: 'Apróbb karcok', valuables_declared: false }));
    for (const s of stages) await step(`order ${o.id} -> ${s}`, () => api('POST', `/orders/${o.id}/stage`, { stage: s }));
  }
  for (const [o, what, due] of [
    [first, 'Hűtőagregát szállítói visszaigazolásra vár', 5],
    [second, 'Ügyfél jóváhagyása a polcrajzra', 3],
    [second, 'Fényezésre váró alkatrész', 9],
  ]) {
    if (o) await step(`blocker ${o.id}`, () => api('POST', `/orders/${o.id}/blockers`, { what, due_date: iso(due), nudge_enabled: false }));
  }

  // ── Finished walkarounds: átvétel (and kiadás) with damages and signatures ──
  // `first` has a signed átvétel and is left waiting for its kiadás on the phone; `third`
  // has both, so the comparison is on screen.
  async function walk(o, kind, who, damages, comment) {
    const zones = (await api('GET', `/inspections/templates?project_type_id=${o.projectTypeId ?? ''}&kind=${kind}`)).items.map((z) => z.zone_key);
    const insp = await api('POST', '/inspections', {
      order_id: o.id,
      kind,
      vehicle_plate: o.spec.plate,
      inspector_name: 'Iroda Ilona',
      driver_name: who,
      location: 'Telephely, Győr',
      odometer: kind === 'checkout' ? 84_200 : 84_260,
      fuel_level: kind === 'checkout' ? '1/2' : '3/4',
      client_key: `seed-${o.id}-${kind}`,
    });
    const made = [];
    for (const [i, d] of damages.entries()) {
      made.push(await api('POST', `/inspections/${insp.id}/damages`, {
        zone_key: zones[(i * 2 + 1) % zones.length],
        damage_type: d.type,
        severity: d.severity,
        note: d.note,
        x: 0.3 + i * 0.2,
        y: 0.4,
        view: 'top',
      }));
    }
    for (const [role, name, seed] of [['inspector', 'Iroda Ilona', 1], ['customer', who, 2]]) {
      const doc = await uploadDocument(o.id, `alairas_${kind}_${role}.png`, signaturePng(seed));
      await api('POST', `/inspections/${insp.id}/signatures`, { role, name, document_id: doc });
    }
    return { insp, made };
  }
  const finish = (id, comment) => api('POST', `/inspections/${id}/sign`, { customer_comment: comment });

  for (const o of [first, third]) {
    if (!o) continue;
    await step(`átvétel ${o.id}`, async () => {
      const out = await walk(o, 'checkout', o.spec.partner.name, [
        { type: 'scratch', severity: 'minor', note: 'Hosszú karc a bal oldali ajtón.' },
        { type: 'dent', severity: 'moderate', note: 'Horpadás a hátsó lökhárítón.' },
      ]);
      await finish(out.insp.id, 'Az átvett állapot megfelel.');
      o.checkout = out;
    });
  }
  const t = third;
  if (t?.checkout) {
    await step(`kiadás ${t.id}`, async () => {
      const out = await walk(t, 'checkin', t.spec.partner.name, [
        { type: 'scratch', severity: 'minor', note: 'Ugyanaz a karc, már átvételkor megvolt.' },
      ]);
      await api('POST', `/inspections/${out.insp.id}/verdicts`, {
        checkin_damage_id: out.made[0].id,
        checkout_damage_id: t.checkout.made[0].id,
        verdict: 'preexisting',
      });
      await finish(out.insp.id, 'Átvettem, minden rendben.');
    });
  }

  // ── Employees with days off and sick leave ──
  const staff = [
    ['Szalai Bence', 'szalai.bence@demo.local', 25, [['annual', -20, -16, 'Nyári szabadság'], ['sick', -3, -1, 'Influenza, orvosi igazolással'], ['annual', 12, 18, 'Családi nyaralás']]],
    ['Varga Eszter', 'varga.eszter@demo.local', 22, [['sick', -10, -6, 'Hátműtét utáni lábadozás'], ['annual', 4, 6, null], ['other', 25, 25, 'Költözés']]],
    ['Horváth Dániel', 'horvath.daniel@demo.local', 20, [['annual', 0, 2, 'Hosszú hétvége'], ['unpaid', 30, 36, 'Fizetés nélküli szabadság'], ['sick', -45, -43, null]]],
    ['Molnár Ágnes', 'molnar.agnes@demo.local', 28, [['sick', 0, 4, 'Orvosi igazolás feltöltve'], ['annual', 40, 49, 'Őszi szabadság'], ['annual', -60, -56, null]]],
  ];
  for (const [name, email, days, absences] of staff) {
    const e = await step(`employee ${name}`, () => api('POST', '/hr/employees', { full_name: name, email, company_phone: '+36 30 555 02' + String(days).padStart(2, '0'), annual_leave_days: days }));
    if (!e) continue;
    for (const [kind, from, to, note] of absences) {
      await step(`absence ${name} ${kind}`, () => api('POST', `/hr/employees/${e.id}/absences`, { kind, start_date: iso(from), end_date: iso(to), note }));
    }
  }

  void bare;
}

await seed();
if (failures.length) {
  console.error(`\nseeded with ${failures.length} problem(s):\n- ${failures.join('\n- ')}`);
  process.exitCode = 1;
} else {
  console.log('showcase data ready');
}
