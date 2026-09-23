import { NavClient, assertCredentials } from '@open-nav/client';
import { startMockServer } from '@open-nav/mock-server';
import type { MockServer } from '@open-nav/mock-server';
import type { Config } from './config.js';

/** A configured client, plus whatever has to be shut down with it. */
export interface NavContext {
  client: NavClient;
  config: Config;
  /** The in-process mock, when `MOCK_MODE=true`. */
  mock?: MockServer;
  close(): Promise<void>;
}

/**
 * Build the NAV client.
 *
 * In mock mode an in-process stand-in is started first and the client is
 * pointed at it. The mock verifies the request signature the way NAV does and
 * runs submitted invoices through the same validator, so an end-to-end test
 * against it exercises the real code path — only the tax authority is fake.
 */
export async function createNavContext(config: Config): Promise<NavContext> {
  let mock: MockServer | undefined;
  let baseUrl = config.baseUrl;

  if (config.mockMode) {
    mock = await startMockServer({
      credentials: config.credentials,
      port: 0,
      validate: true,
      taxpayers: [
        {
          taxNumber: config.credentials.taxNumber.slice(0, 8),
          name: 'Mock taxpayer (MOCK_MODE)',
          valid: true,
          vatCode: '2',
          countyCode: '41',
        },
      ],
    });
    baseUrl = mock.url;
    console.warn(
      `[nav-sidecar] MOCK_MODE is on: talking to an in-process fake NAV at ${mock.url}. ` +
        'Nothing is reported to the tax authority.',
    );
  } else {
    // NAV answers every credential problem with the same opaque
    // INVALID_SECURITY_USER, so shape errors are worth catching at startup.
    assertCredentials(config.credentials);
  }

  const client = new NavClient({
    credentials: config.credentials,
    software: config.software,
    environment: config.environment,
    ...(baseUrl ? { baseUrl } : {}),
    ...(config.requestIdPrefix ? { requestIdPrefix: config.requestIdPrefix } : {}),
    transport: { timeoutMs: config.navTimeoutMs },
  });

  return {
    client,
    config,
    ...(mock ? { mock } : {}),
    async close() {
      await mock?.close();
    },
  };
}
