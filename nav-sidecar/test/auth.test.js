/**
 * The sidecar acts under the company's NAV credentials, so every route but
 * `/health` demands the caller token, and the service refuses to start without
 * one outside mock mode.
 *
 * Run with: npm test
 */
import assert from 'node:assert/strict';
import { after, before, describe, it } from 'node:test';

const { startSidecar } = await import('../dist/index.js');
const { loadConfig, ConfigError } = await import('../dist/config.js');

const TOKEN = 'test-sidecar-token-0123456789abcdef';

describe('caller token', () => {
  /** @type {Awaited<ReturnType<typeof startSidecar>>} */
  let sidecar;

  before(async () => {
    sidecar = await startSidecar(
      loadConfig({ MOCK_MODE: 'true', PORT: '0', SIDECAR_TOKEN: TOKEN }),
    );
  });
  after(async () => sidecar?.close());

  const get = (path, headers = {}) => fetch(`${sidecar.url}${path}`, { headers });

  it('listens on loopback unless told otherwise', () => {
    assert.equal(sidecar.config.host, '127.0.0.1');
  });

  it('keeps /health open for the container runtime', async () => {
    assert.equal((await get('/health')).status, 200);
  });

  it('refuses a caller without the token, before anything reaches NAV', async () => {
    const response = await get('/invoices/AT2026-0001');
    assert.equal(response.status, 401);
    const body = await response.json();
    assert.equal(body.error.kind, 'unauthorized');
  });

  it('refuses a wrong token', async () => {
    const response = await get('/invoices/AT2026-0001', { authorization: `Bearer ${TOKEN}x` });
    assert.equal(response.status, 401);
  });

  it('lets the right token through to the routes', async () => {
    const response = await get('/invoices/AT2026-0001', { authorization: `Bearer ${TOKEN}` });
    assert.notEqual(response.status, 401);
  });
});

describe('startup', () => {
  const real = {
    NAV_LOGIN: 'login',
    NAV_PASSWORD: 'password',
    NAV_SIGN_KEY: 'sign-key',
    NAV_EXCHANGE_KEY: '0123456789abcdef',
    NAV_TAX_NUMBER: '12345678',
    NAV_SOFTWARE_ID: 'HU12345678-AUTOCRM',
    NAV_SOFTWARE_NAME: 'AutoCRM',
    NAV_SOFTWARE_DEV_CONTACT: 'dev@example.hu',
  };

  it('refuses to run with real credentials and no token', () => {
    assert.throws(() => loadConfig(real), ConfigError);
  });

  it('refuses a short token', () => {
    assert.throws(() => loadConfig({ ...real, SIDECAR_TOKEN: 'short' }), ConfigError);
  });

  it('starts with real credentials and a token', () => {
    assert.equal(loadConfig({ ...real, SIDECAR_TOKEN: TOKEN }).callerToken, TOKEN);
  });
});
