// Cross-checks every call in the Android AutoCrmApi against openapi/openapi.json: paths,
// methods, query parameters, request fields (unknown / required-but-missing) and response
// fields the phone treats as non-null but the server may omit or send as null (those
// crash the screen with "unparseable response"). Exits 1 when any ERROR is found.
const fs = require('fs');
// Usage (from the repository root): node android/scripts/check-api-contract.js .
const ROOT = process.argv[2] ?? '.';
const api = fs.readFileSync(ROOT + '/android/app/src/main/java/hu/autotherm/autocrm/data/api/AutoCrmApi.kt', 'utf8').replace(/\r\n/g, '\n');
const dto = fs.readFileSync(ROOT + '/android/app/src/main/java/hu/autotherm/autocrm/data/api/Dto.kt', 'utf8').replace(/\r\n/g, '\n');
const oa = JSON.parse(fs.readFileSync(ROOT + '/openapi/openapi.json', 'utf8'));
const S = oa.components.schemas;

// ── Kotlin DTOs ──
const classes = {};
const classRe = /data class (\w+)\(/g;
let m;
const stripComments = s => s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
const src = stripComments(dto);
while ((m = classRe.exec(src))) {
  // balanced parens from the opening one
  let i = m.index + m[0].length, depth = 1, inStr = false;
  const start = i;
  for (; i < src.length && depth > 0; i++) {
    const c = src[i];
    if (c === '"' && src[i - 1] !== '\\') inStr = !inStr;
    if (inStr) continue;
    if (c === '(' || c === '<' && false) depth++;
    else if (c === ')') depth--;
  }
  const body = src.slice(start, i - 1);
  // split on top-level commas
  const parts = [];
  let d = 0, cur = '', s2 = false;
  for (let j = 0; j < body.length; j++) {
    const c = body[j];
    if (c === '"' && body[j - 1] !== '\\') s2 = !s2;
    if (!s2) {
      if ('(<{['.includes(c)) d++;
      else if (')>}]'.includes(c)) d--;
      else if (c === ',' && d === 0) { parts.push(cur); cur = ''; continue; }
    }
    cur += c;
  }
  parts.push(cur);
  const fields = [];
  for (const p of parts) {
    const f = p.match(/(?:@SerialName\("([^"]+)"\)\s*)?(?:@\w+(?:\([^)]*\))?\s*)*va[lr] (\w+)\s*:\s*([^=]+?)\s*(=[\s\S]*)?$/);
    if (!f) continue;
    let type = f[3].trim();
    let nullable = false;
    if (type.endsWith('?')) { nullable = true; type = type.slice(0, -1); }
    fields.push({ name: f[1] || f[2], prop: f[2], type, nullable, hasDefault: !!f[4] });
  }
  classes[m[1]] = fields;
}

// ── API functions ──
const fns = [];
const chunks = api.split(/\n    suspend fun /).slice(1);
for (const c of chunks) {
  const name = c.match(/^(?:<[^>]+>\s*)?(\w+)/)[1];
  const sig = c.slice(0, c.indexOf(')') + 1);
  const end = c.search(/\n    (\/\/ ──|\/\*\*|private|suspend)/);
  const text = end > 0 ? c.slice(0, end) : c;
  let paths = [];
  const urlm = text.match(/url\(([^\n]*?)\)\s*(?:\.build|\.add|\n)/g) || [];
  for (const u of (text.match(/url\((?:if \([^)]*\) )?"[^"]+"(?: else "[^"]+")?/g) || [])) {
    for (const p of (u.match(/"(\/[^"]*)"/g) || [])) paths.push(p.slice(1, -1));
  }
  if (name === 'decodeVin') paths = ['/vehicles/decode/{vin}'];
  if (name === 'setEmployeeArchived') paths = ['/hr/employees/$id/archive', '/hr/employees/$id/unarchive'];
  const method = /\.get\(\)/.test(text) ? 'get' : /\.post\(/.test(text) ? 'post' : /\.patch\(/.test(text) ? 'patch' : /\.put\(/.test(text) ? 'put' : /\.delete\(\)/.test(text) ? 'delete' : '?';
  const query = [...text.matchAll(/addQueryParameter\("(\w+)"/g)].map(x => x[1]);
  let reqType = null;
  const bm = text.match(/body\(\s*(\w+)\(/);
  if (bm && bm[1] !== 'body') reqType = bm[1];
  else {
    const bv = text.match(/body\((\w+)\)/);
    if (bv) {
      const pm = sig.match(new RegExp(bv[1] + ':\\s*([\\w.]+)'));
      reqType = pm ? pm[1].split('.').pop() : '?' + bv[1];
    } else if (/toRequestBody\(jsonMedia\)/.test(text)) reqType = 'JsonObject';
  }
  if (name === 'requestDocumentUpload') reqType = 'UploadRequest';
  let resType = null;
  const im = text.match(/Items\.serializer\((\w+)\.serializer\(\)\)/);
  const lm = text.match(/ListSerializer\((\w+)\.serializer\(\)\)/);
  if (im) resType = { items: im[1] };
  else if (lm) resType = { list: lm[1] };
  else {
    const sm = [...text.matchAll(/(\w+)\.serializer\(\)/g)].map(x => x[1]).filter(x => x !== reqType);
    if (sm.length) resType = { one: sm[sm.length - 1] };
  }
  for (const p of paths) fns.push({ name, path: p, method, query, reqType, resType });
}

const norm = p => p.replace(/\$\{[^}]+\}|\$\w+/g, '{}').replace(/\{[^}]+\}/g, '{}');
const oaIndex = {};
for (const [p, ops] of Object.entries(oa.paths)) oaIndex[norm(p)] = { p, ops };

const deref = s => {
  while (s && s.$ref) s = S[s.$ref.split('/').pop()];
  return s;
};
const refName = s => s && s.$ref ? s.$ref.split('/').pop() : null;
function schemaNullable(p) {
  if (!p) return false;
  if (Array.isArray(p.type) && p.type.includes('null')) return true;
  if (p.oneOf && p.oneOf.some(x => x.type === 'null')) return true;
  if (p.anyOf && p.anyOf.some(x => x.type === 'null')) return true;
  if (p.nullable) return true;
  return false;
}
function inner(p) {
  // the non-null schema of a property
  if (p.oneOf) { const x = p.oneOf.find(x => x.type !== 'null'); if (x) return x; }
  if (p.anyOf) { const x = p.anyOf.find(x => x.type !== 'null'); if (x) return x; }
  if (p.allOf && p.allOf.length === 1) return p.allOf[0];
  return p;
}
function baseType(p) {
  p = inner(p);
  const d = deref(p) || {};
  let t = d.type;
  if (Array.isArray(t)) t = t.find(x => x !== 'null');
  if (!t && d.enum) t = 'string';
  if (!t && d.oneOf) t = 'oneOf';
  return { t, d, ref: refName(p) };
}
const KT = { String: 'string', Long: 'integer', Int: 'integer', Boolean: 'boolean', Double: 'number', Float: 'number' };

const issues = [];
const seen = new Set();
function compare(kName, schema, dir, ctx) {
  const key = kName + '|' + dir + '|' + JSON.stringify(schema).slice(0, 80);
  if (seen.has(key)) return;
  seen.add(key);
  const fields = classes[kName];
  if (!fields) { issues.push(`${ctx}: Kotlin class ${kName} not parsed`); return; }
  const d = deref(schema);
  if (!d) { issues.push(`${ctx}: no schema for ${kName}`); return; }
  let props = d.properties || {};
  let required = d.required || [];
  if (d.oneOf && d.oneOf.every(v => deref(v) && deref(v).type === 'object')) {
    const vs = d.oneOf.map(deref);
    for (const v of vs) props = { ...props, ...(v.properties || {}) };
    required = vs.map(v => v.required || []).reduce((a, b) => a.filter(x => b.includes(x)));
  }
  if (d.allOf) for (const part of d.allOf) { const x = deref(part); props = { ...props, ...(x.properties || {}) }; required = required.concat(x.required || []); }
  const sName = refName(schema) || '(inline)';
  for (const f of fields) {
    const p = props[f.name];
    if (!p) {
      if (dir === 'res' && !f.nullable && !f.hasDefault) issues.push(`ERROR ${ctx}: ${kName}.${f.name} required by app, absent from server ${sName}`);
      else if (dir === 'req') issues.push(`WARN  ${ctx}: ${kName}.${f.name} sent but server ${sName} has no such field (ignored)`);
      else issues.push(`info  ${ctx}: ${kName}.${f.name} not in server ${sName} (always default)`);
      continue;
    }
    const req = required.includes(f.name);
    const nul = schemaNullable(p);
    if (dir === 'res' && !f.nullable && (nul)) issues.push(`ERROR ${ctx}: ${kName}.${f.name} non-null in app, server ${sName} may send null`);
    if (dir === 'res' && !f.nullable && !f.hasDefault && !req && !nul) issues.push(`WARN  ${ctx}: ${kName}.${f.name} non-null no default, server ${sName} marks optional`);
    // types
    const { t, d: pd, ref } = baseType(p);
    let kt = f.type.replace(/\s/g, '');
    const lst = kt.match(/^List<(\w+)\??>$/);
    const map = kt.match(/^Map<String,(\w+)\??>$/);
    if (lst) {
      if (t !== 'array') issues.push(`ERROR ${ctx}: ${kName}.${f.name} List in app, server type ${t}`);
      else {
        const it = (inner(p).items) || (pd.items);
        const ik = lst[1];
        if (KT[ik]) { const bt = baseType(it).t; if (bt !== KT[ik] && !(KT[ik] === 'number' && bt === 'integer')) issues.push(`ERROR ${ctx}: ${kName}.${f.name} List<${ik}> vs server items ${bt}`); }
        else if (classes[ik]) compare(ik, it, dir, `${ctx} > ${f.name}[]`);
      }
    } else if (map) {
      if (t !== 'object') issues.push(`WARN  ${ctx}: ${kName}.${f.name} Map vs ${t}`);
    } else if (KT[kt]) {
      if (t !== KT[kt] && !(KT[kt] === 'number' && t === 'integer')) {
        if (!(kt === 'String' && (t === 'oneOf' || t === undefined))) issues.push(`ERROR ${ctx}: ${kName}.${f.name} ${kt} in app, server ${t}${ref ? ' (' + ref + ')' : ''}`);
      }
    } else if (classes[kt]) {
      compare(kt, inner(p), dir, `${ctx} > ${f.name}`);
    } else if (kt === 'JsonElement' || kt === 'JsonObject' || kt.startsWith('kotlinx')) {
      // free-form
    } else {
      issues.push(`info  ${ctx}: ${kName}.${f.name} type ${kt} unchecked`);
    }
  }
  if (dir === 'req') {
    for (const r of required) {
      const f = fields.find(x => x.name === r);
      if (!f) issues.push(`ERROR ${ctx}: server ${sName} requires ${r}, app ${kName} never sends it`);
      else if (f.nullable) issues.push(`WARN  ${ctx}: server ${sName} requires ${r}, app ${kName}.${f.prop} is nullable (omitted when null)`);
    }
  }
}

const report = [];
for (const f of fns) {
  const hit = oaIndex[norm(f.path)];
  const ctx = `${f.name} ${f.method.toUpperCase()} ${f.path}`;
  if (!hit) { issues.push(`ERROR ${ctx}: path not in OpenAPI`); continue; }
  const op = hit.ops[f.method];
  if (!op) { issues.push(`ERROR ${ctx}: method not in OpenAPI (has ${Object.keys(hit.ops)})`); continue; }
  const params = (op.parameters || []).filter(p => p.in === 'query').map(p => p.name);
  for (const q of f.query) if (!params.includes(q)) issues.push(`ERROR ${ctx}: query ?${q} unknown (server has ${params.join(',')})`);
  const rb = op.requestBody && op.requestBody.content && (op.requestBody.content['application/json'] || Object.values(op.requestBody.content)[0]);
  if (f.reqType && f.reqType !== 'JsonObject') {
    if (!rb) issues.push(`WARN  ${ctx}: app sends ${f.reqType}, server declares no body`);
    else compare(f.reqType, rb.schema, 'req', ctx);
  } else if (f.reqType === 'JsonObject') {
    report.push(`manual ${ctx}: JsonObject body vs ${rb ? refName(rb.schema) : 'none'}`);
  } else if (rb && op.requestBody.required) {
    issues.push(`WARN  ${ctx}: server expects a body (${refName(rb.schema)}), app sends none`);
  }
  if (op.requestBody && op.requestBody.description) report.push(`note   ${ctx}: ${op.requestBody.description.replace(/\n/g, ' ')}`);
  const ok = Object.entries(op.responses).find(([c]) => /^2/.test(c));
  if (f.resType && ok) {
    const sch = ok[1].content && Object.values(ok[1].content)[0] && Object.values(ok[1].content)[0].schema;
    if (!sch) { issues.push(`ERROR ${ctx}: app parses a body, server ${ok[0]} has none`); continue; }
    if (f.resType.items) {
      const d = deref(sch);
      const it = d && d.properties && d.properties.items && d.properties.items.items;
      if (!it) issues.push(`ERROR ${ctx}: app expects {items:[...]}, server sends ${refName(sch) || d && d.type}`);
      else compare(f.resType.items, it, 'res', ctx);
    } else if (f.resType.list) {
      const d = deref(sch);
      if (d.type !== 'array') issues.push(`ERROR ${ctx}: app expects array, server sends ${refName(sch)}`);
      else compare(f.resType.list, d.items, 'res', ctx);
    } else compare(f.resType.one, sch, 'res', ctx);
  }
}
console.log(`${fns.length} calls checked\n`);
const order = s => s.startsWith('ERROR') ? 0 : s.startsWith('WARN') ? 1 : 2;
[...new Set(issues)].sort((a, b) => order(a) - order(b)).forEach(x => console.log(x));
console.log('\n--- manual / notes ---');
report.forEach(x => console.log(x));
if (issues.some(x => x.startsWith('ERROR'))) process.exit(1);
