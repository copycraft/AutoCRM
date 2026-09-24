// Writes the ERROR_TEXT table in Android's Errors.kt from openapi.json's x-error-catalog.
const fs = require('fs');
// Usage (from the repository root): node android/scripts/gen-error-text.js .
const root = process.argv[2] ?? '.';
const cat = JSON.parse(fs.readFileSync(`${root}/openapi/openapi.json`, 'utf8'))
  .components.schemas.ErrorCode['x-error-catalog'];
const esc = (s) => s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\$/g, '\\$');
const rows = cat.map((c) => `    "${c.code}" to "${esc(c.hu)}",`).join('\n');
const p = `${root}/android/app/src/main/java/hu/autotherm/autocrm/ui/common/Errors.kt`;
let s = fs.readFileSync(p, 'utf8');
const marker = '\n/**\n * The backend error-code catalog';
if (s.includes(marker)) s = s.slice(0, s.indexOf(marker)) + '\n';
s = s.replace(
  '    is ApiException.Rule -> e.detail ?: e.code',
  `    // Validation carries the field-level reason in its detail; every other code reads
    // from the shared catalog so the phone and the web say the same thing.
    is ApiException.Rule ->
        if (e.code == "validation") e.detail ?: ERROR_TEXT.getValue("validation")
        else ERROR_TEXT[e.code] ?: e.detail ?: e.code`,
);
s += `
/**
 * The backend error-code catalog (\`backend/src/error.rs::ERROR_CODES\`, published in
 * \`openapi.json\` as \`ErrorCode\` with \`x-error-catalog\`), Hungarian text only.
 * \`ErrorCatalogTest\` fails when a code is missing here or its text drifts.
 */
internal val ERROR_TEXT: Map<String, String> = mapOf(
${rows}
)
`;
fs.writeFileSync(p, s);
