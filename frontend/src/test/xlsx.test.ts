// The .xlsx writer: a valid stored zip holding the sheet, numbers kept as numbers,
// text escaped, and the TSV that pastes into Google Sheets.
import { describe, expect, it } from 'vitest';
import { buildXlsx, colName, toTsv } from '@/lib/utils/xlsx';

const text = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

describe('xlsx export', () => {
  it('names columns like a spreadsheet', () => {
    expect([0, 25, 26, 27, 701, 702].map(colName)).toEqual(['A', 'Z', 'AA', 'AB', 'ZZ', 'AAA']);
  });

  it('writes a zip with the workbook parts and the cells', () => {
    const bytes = buildXlsx('Leadek', ['Cím', 'Összeg'], [['Hűtős <Sprinter> & társa', 1234.5], [null, 7]]);
    // Local file header signature, and the end-of-central-directory record.
    expect(Array.from(bytes.slice(0, 4))).toEqual([0x50, 0x4b, 0x03, 0x04]);
    const body = text(bytes);
    for (const part of ['[Content_Types].xml', 'xl/workbook.xml', 'xl/worksheets/sheet1.xml', 'xl/styles.xml']) {
      expect(body).toContain(part);
    }
    expect(body).toContain('<sheet name="Leadek"');
    expect(body).toContain('Hűtős &lt;Sprinter&gt; &amp; társa');
    expect(body).toContain('<c r="B2"><v>1234.5</v></c>');
    expect(body).toContain('<c r="B3"><v>7</v></c>');
    // The empty cell is left out, not written as an empty string.
    expect(body).not.toContain('r="A3"');
    expect(body).toContain('<autoFilter ref="A1:B3"/>');
  });

  it('pastes as clean columns', () => {
    expect(toTsv(['a', 'b'], [['x\ty', 'line\nbreak'], [1, null]])).toBe('a\tb\nx y\tline break\n1\t');
  });
});
