/**
 * End-to-end test against the bundled mock NAV service.
 *
 * Nothing here needs credentials: `MOCK_MODE=true` starts an in-process stand-in
 * that verifies the request signature the way NAV does and runs submitted
 * invoices through the same validator, so the whole path — build, validate,
 * sign, submit, poll, render — is exercised for real.
 *
 * Run with: npm test
 */
import assert from 'node:assert/strict';
import { after, before, describe, it } from 'node:test';

process.env.MOCK_MODE = 'true';
process.env.PORT = '0';
process.env.HOST = '127.0.0.1';
// Keep the polling loop brisk; the mock settles a transaction immediately.
process.env.NAV_POLL_INITIAL_DELAY_MS = '10';

const { startSidecar } = await import('../dist/index.js');

const TODAY = new Date().toISOString().slice(0, 10);

/** The mock's taxpayer. The supplier must be whoever the request authenticates as. */
const SUPPLIER = {
  name: 'Autotherm Kft',
  taxNumber: '99999999',
  bankAccount: '12345678-12345678-12345678',
  address: {
    postalCode: '1117',
    city: 'Budapest',
    streetName: 'Kossuth',
    publicPlaceCategory: 'utca',
    number: '12',
  },
};

const CUSTOMER = {
  name: 'Beszerző Kft',
  taxNumber: '99887764',
  address: {
    postalCode: '6000',
    city: 'Kecskemét',
    streetName: 'Petőfi',
    publicPlaceCategory: 'tér',
    number: '3',
  },
};

function invoicePayload(invoiceNumber) {
  return {
    invoiceNumber,
    issueDate: TODAY,
    deliveryDate: TODAY,
    paymentDate: TODAY,
    currency: 'HUF',
    paymentMethod: 'TRANSFER',
    supplier: SUPPLIER,
    customer: CUSTOMER,
    lines: [
      {
        description: 'Klímaberendezés telepítése',
        quantity: 1,
        unit: 'PIECE',
        unitPrice: '100000',
        vatPercentage: '0.27',
        nature: 'SERVICE',
      },
      {
        description: 'Rézcső',
        quantity: '2.5',
        unit: 'METER',
        unitPrice: '4000',
        vatPercentage: '0.27',
      },
    ],
  };
}

describe('nav-sidecar against the mock NAV service', () => {
  let sidecar;

  const call = async (method, path, body) => {
    const response = await fetch(`${sidecar.url}${path}`, {
      method,
      ...(body === undefined
        ? {}
        : { headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }),
    });
    const type = response.headers.get('content-type') ?? '';
    return {
      status: response.status,
      type,
      body: type.includes('application/json')
        ? await response.json()
        : Buffer.from(await response.arrayBuffer()),
    };
  };

  before(async () => {
    sidecar = await startSidecar();
  });

  after(async () => {
    await sidecar?.close();
  });

  it('answers /health without credentials or a NAV call', async () => {
    const { status, body } = await call('GET', '/health');
    assert.equal(status, 200);
    assert.equal(body.status, 'ok');
    assert.equal(body.mockMode, true);
    // Nothing secret leaks into the liveness check.
    assert.equal(JSON.stringify(body).includes('mock-password'), false);
  });

  it('creates an invoice and reports the transaction', async () => {
    const { status, body } = await call('POST', '/invoices', invoicePayload('SIDECAR-TEST-001'));
    assert.equal(status, 201, JSON.stringify(body));
    assert.equal(body.invoiceNumber, 'SIDECAR-TEST-001');
    assert.equal(body.operation, 'CREATE');
    assert.equal(body.accepted, true);
    assert.equal(body.status, 'DONE');
    assert.match(body.transactionId, /^MOCKTX/);
    assert.deepEqual(body.totals, {
      currency: 'HUF',
      net: '110000.00',
      vat: '29700.00',
      gross: '139700.00',
    });
    assert.ok(Array.isArray(body.messages));
  });

  it('renders the reported invoice as a PDF read back from NAV', async () => {
    const { status, type, body } = await call('GET', '/invoices/SIDECAR-TEST-001/pdf');
    assert.equal(status, 200);
    assert.equal(type, 'application/pdf');
    assert.equal(body.subarray(0, 5).toString('latin1'), '%PDF-');
  });

  it('reads back what NAV holds for a number, so a caller can reconcile an unknown outcome', async () => {
    const { status, body } = await call('GET', '/invoices/SIDECAR-TEST-001');
    assert.equal(status, 200, JSON.stringify(body));
    assert.equal(body.invoiceNumber, 'SIDECAR-TEST-001');
    assert.equal(body.issueDate, TODAY);
    assert.deepEqual(body.totals, {
      currency: 'HUF',
      net: '110000.00',
      vat: '29700.00',
      gross: '139700.00',
    });
  });

  it('answers 404 for a number NAV does not hold', async () => {
    const { status, body } = await call('GET', '/invoices/NEVER-REPORTED-999');
    assert.equal(status, 404, JSON.stringify(body));
    assert.equal(body.error.kind, 'not_found');
  });

  it('stornoes an invoice, fetching the original from NAV', async () => {
    const { status, body } = await call('POST', '/invoices/SIDECAR-TEST-001/storno', {
      stornoInvoiceNumber: 'SIDECAR-TEST-001-S',
    });
    assert.equal(status, 201, JSON.stringify(body));
    assert.equal(body.operation, 'STORNO');
    assert.equal(body.invoiceNumber, 'SIDECAR-TEST-001-S');
    assert.equal(body.originalInvoiceNumber, 'SIDECAR-TEST-001');
    assert.equal(body.accepted, true);
    // A storno reverses the amounts.
    assert.equal(body.totals.net, '-110000.00');
  });

  it('reads a storno back under its own number, with the reversed totals', async () => {
    // What the backend compares against when a retried storno meets a duplicate number.
    const { status, body } = await call('GET', '/invoices/SIDECAR-TEST-001-S');
    assert.equal(status, 200, JSON.stringify(body));
    assert.equal(body.invoiceNumber, 'SIDECAR-TEST-001-S');
    assert.deepEqual(body.totals, {
      currency: 'HUF',
      net: '-110000.00',
      vat: '-29700.00',
      gross: '-139700.00',
    });
  });


  it('refuses a storno whose supplied original is a different invoice', async () => {
    const { status, body } = await call('POST', '/invoices/SIDECAR-TEST-001/storno', {
      stornoInvoiceNumber: 'SIDECAR-TEST-001-S2',
      original: invoicePayload('SOMETHING-ELSE'),
    });
    assert.equal(status, 400);
    assert.equal(body.error.kind, 'bad_request');
  });

  it('annuls a reported invoice', async () => {
    await call('POST', '/invoices', invoicePayload('SIDECAR-TEST-002'));
    const { status, body } = await call('POST', '/invoices/SIDECAR-TEST-002/annul', {
      code: 'ERRATIC_DATA',
      reason: 'A vevő adószáma hibás volt',
    });
    assert.equal(status, 201, JSON.stringify(body));
    assert.equal(body.operation, 'ANNUL');
    assert.equal(body.accepted, true);
    assert.match(body.transactionId, /^MOCKAN/);
  });

  it("surfaces NAV's own fault code rather than swallowing it", async () => {
    // The mock implements no queryInvoiceChainDigest, so it answers the way NAV
    // does for an unknown operation — which is exactly what should reach the
    // caller, code and all.
    const { status, body } = await call('GET', '/invoices/SIDECAR-TEST-001/chain');
    assert.equal(status, 502);
    assert.equal(body.error.kind, 'nav_error');
    assert.equal(body.error.navErrorCode, 'INVALID_REQUEST');
    assert.ok(Array.isArray(body.error.messages));
  });

  it('rejects an invoice NAV would reject, before submitting it', async () => {
    const payload = invoicePayload('SIDECAR-TEST-003');
    // Reporting for a taxpayer other than the authenticated one.
    payload.supplier = { ...SUPPLIER, taxNumber: '12345678' };
    const { status, body } = await call('POST', '/invoices', payload);
    assert.equal(status, 422);
    assert.equal(body.error.kind, 'validation');
    assert.ok(body.error.messages.length > 0);
    assert.ok(body.error.messages.every((message) => typeof message.message === 'string'));
  });

  it('rejects an unknown field instead of ignoring it', async () => {
    const { status, body } = await call('POST', '/invoices', {
      ...invoicePayload('SIDECAR-TEST-004'),
      navPassword: 'hunter2',
    });
    assert.equal(status, 400);
    assert.equal(body.error.kind, 'bad_request');
    assert.ok(body.error.messages.some((message) => message.path === 'navPassword'));
  });

  it('rejects a VAT rate given as a percentage', async () => {
    const payload = invoicePayload('SIDECAR-TEST-005');
    payload.lines[0].vatPercentage = 27;
    const { status, body } = await call('POST', '/invoices', payload);
    assert.equal(status, 400);
    assert.match(JSON.stringify(body), /fraction/);
  });

  describe('proformas, which never reach NAV', () => {
    it('renders a proforma and serves it by id', async () => {
      const created = await call('POST', '/proformas', {
        ...invoicePayload('DIJBEKERO-001'),
        id: 'proforma-1',
        note: 'Kérjük a fenti összeget 8 napon belül átutalni.',
      });
      assert.equal(created.status, 201, JSON.stringify(created.body));
      assert.equal(created.body.reportedToNav, false);
      assert.equal(created.body.documentType, 'proforma');
      assert.equal(created.body.id, 'proforma-1');
      assert.equal(created.body.totals.gross, '139700.00');
      assert.equal(Buffer.from(created.body.pdfBase64, 'base64').subarray(0, 5).toString('latin1'), '%PDF-');

      const fetched = await call('GET', '/proformas/proforma-1/pdf');
      assert.equal(fetched.status, 200);
      assert.equal(fetched.type, 'application/pdf');
      assert.equal(fetched.body.subarray(0, 5).toString('latin1'), '%PDF-');
    });

    it('opens no NAV transaction for a proforma', async () => {
      const before = sidecarMockState().transactions.size;
      await call('POST', '/proformas', invoicePayload('DIJBEKERO-002'));
      assert.equal(sidecarMockState().transactions.size, before);
    });

    it('says plainly that an unknown proforma id is not held', async () => {
      const { status, body } = await call('GET', '/proformas/never-rendered/pdf');
      assert.equal(status, 404);
      assert.equal(body.error.kind, 'not_found');
      assert.match(body.error.message, /POST \/proformas/);
    });
  });

  it('404s an unknown route in the documented envelope', async () => {
    const { status, body } = await call('GET', '/nope');
    assert.equal(status, 404);
    assert.equal(body.error.kind, 'not_found');
    assert.deepEqual(body.error.messages, []);
  });

  /** The mock records everything it received, which is how we prove a negative. */
  function sidecarMockState() {
    return sidecar.mock.state;
  }
});
