import { assertSoftwareId } from '@open-nav/core';
import type { NavEnvironment, SoftwareType } from '@open-nav/core';
import type { NavCredentials } from '@open-nav/client';

/**
 * Everything the sidecar needs, read from the environment and nowhere else.
 *
 * Credentials are deliberately not reachable from a request: a caller that
 * could name its own technical user would turn this service into an open relay
 * for reporting invoices under someone else's tax number.
 */
export interface Config {
  host: string;
  port: number;
  /** Start an in-process mock NAV service and talk to that instead. */
  mockMode: boolean;
  environment: NavEnvironment;
  /** Overrides the environment's base URL. Set for us by mock mode. */
  baseUrl?: string;
  credentials: NavCredentials;
  software: SoftwareType;
  /** Prefix for generated NAV request identifiers, for tracing. */
  requestIdPrefix?: string;
  /** Per-attempt HTTP timeout towards NAV. */
  navTimeoutMs: number;
  /** How long to poll a transaction before giving up. */
  pollTimeoutMs: number;
  /** First delay before polling a transaction. */
  pollInitialDelayMs: number;
  /** Document language for rendered PDFs. */
  pdfLanguage: 'hu' | 'en' | 'de';
  /** How long a rendered proforma stays retrievable by id. */
  proformaTtlMs: number;
  /** Most proformas kept in memory at once. */
  proformaCacheMax: number;
  /** Largest request body accepted. */
  bodyLimit: string;
}

/**
 * Credentials the bundled mock accepts. Obviously fake, and only ever used when
 * `MOCK_MODE=true`, so a misconfigured production deployment cannot fall back
 * to them silently — it fails at startup instead.
 */
const MOCK_CREDENTIALS: NavCredentials = {
  login: 'mocklogin123',
  password: 'mock-password',
  signKey: 'mock-sign-key-0123456789',
  exchangeKey: '0123456789abcdef',
  taxNumber: '99999999',
};

export class ConfigError extends Error {}

/** The environment the helpers below read; set once at the top of `loadConfig`. */
let source: NodeJS.ProcessEnv = process.env;

function required(name: string, fallback?: string): string {
  const value = source[name]?.trim();
  if (value) return value;
  if (fallback !== undefined) return fallback;
  throw new ConfigError(`${name} is required`);
}

function optional(name: string): string | undefined {
  const value = source[name]?.trim();
  return value ? value : undefined;
}

function number(name: string, fallback: number): number {
  const raw = optional(name);
  if (raw === undefined) return fallback;
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) {
    throw new ConfigError(`${name} must be a positive number, got ${JSON.stringify(raw)}`);
  }
  return value;
}

function port(name: string, fallback: number): number {
  const raw = optional(name);
  if (raw === undefined) return fallback;
  const value = Number(raw);
  // 0 is meaningful: it asks the OS for a free port, which the tests rely on.
  if (!Number.isInteger(value) || value < 0 || value > 65_535) {
    throw new ConfigError(`${name} must be a port number between 0 and 65535, got ${JSON.stringify(raw)}`);
  }
  return value;
}

function boolean(name: string, fallback: boolean): boolean {
  const raw = optional(name)?.toLowerCase();
  if (raw === undefined) return fallback;
  if (['true', '1', 'yes', 'on'].includes(raw)) return true;
  if (['false', '0', 'no', 'off'].includes(raw)) return false;
  throw new ConfigError(`${name} must be true or false, got ${JSON.stringify(raw)}`);
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  source = env;
  const mockMode = boolean('MOCK_MODE', false);

  const environmentRaw = source.NAV_ENVIRONMENT?.trim() || 'test';
  if (environmentRaw !== 'test' && environmentRaw !== 'production') {
    throw new ConfigError(`NAV_ENVIRONMENT must be test or production, got ${JSON.stringify(environmentRaw)}`);
  }

  // In mock mode every credential has a stand-in, so the service starts with no
  // secrets at all. Outside it, all five are required.
  const credentials: NavCredentials = mockMode
    ? {
        login: required('NAV_LOGIN', MOCK_CREDENTIALS.login),
        password: required('NAV_PASSWORD', MOCK_CREDENTIALS.password),
        signKey: required('NAV_SIGN_KEY', MOCK_CREDENTIALS.signKey),
        exchangeKey: required('NAV_EXCHANGE_KEY', MOCK_CREDENTIALS.exchangeKey),
        taxNumber: required('NAV_TAX_NUMBER', MOCK_CREDENTIALS.taxNumber),
      }
    : {
        login: required('NAV_LOGIN'),
        password: required('NAV_PASSWORD'),
        signKey: required('NAV_SIGN_KEY'),
        exchangeKey: required('NAV_EXCHANGE_KEY'),
        taxNumber: required('NAV_TAX_NUMBER'),
      };

  const softwareId = required('NAV_SOFTWARE_ID', mockMode ? 'AUTOCRM-SIDECAR-01' : undefined);
  // NAV's own rule: exactly 18 characters of [0-9A-Z-]. Checked here so the
  // failure names the field, rather than arriving as an opaque rejection.
  assertSoftwareId(softwareId);

  const softwareName = required('NAV_SOFTWARE_NAME', mockMode ? 'autocrm-nav-sidecar' : undefined);

  const software: SoftwareType = {
    softwareId,
    softwareName,
    softwareOperation: optional('NAV_SOFTWARE_OPERATION') === 'ONLINE_SERVICE' ? 'ONLINE_SERVICE' : 'LOCAL_SOFTWARE',
    softwareMainVersion: required('NAV_SOFTWARE_VERSION', '1.0'),
    softwareDevName: required('NAV_SOFTWARE_DEV_NAME', softwareName),
    softwareDevContact: required('NAV_SOFTWARE_DEV_CONTACT', mockMode ? 'mock@example.invalid' : undefined),
    ...(optional('NAV_SOFTWARE_DEV_COUNTRY_CODE')
      ? { softwareDevCountryCode: optional('NAV_SOFTWARE_DEV_COUNTRY_CODE') as string }
      : {}),
    ...(optional('NAV_SOFTWARE_DEV_TAX_NUMBER')
      ? { softwareDevTaxNumber: optional('NAV_SOFTWARE_DEV_TAX_NUMBER') as string }
      : {}),
  };

  const pdfLanguageRaw = (optional('PDF_LANGUAGE') ?? 'hu').toLowerCase();
  if (pdfLanguageRaw !== 'hu' && pdfLanguageRaw !== 'en' && pdfLanguageRaw !== 'de') {
    throw new ConfigError(`PDF_LANGUAGE must be hu, en or de, got ${JSON.stringify(pdfLanguageRaw)}`);
  }

  const baseUrl = optional('NAV_BASE_URL');

  return {
    host: optional('HOST') ?? '0.0.0.0',
    port: port('PORT', 8080),
    mockMode,
    environment: environmentRaw,
    ...(baseUrl ? { baseUrl } : {}),
    credentials,
    software,
    ...(optional('NAV_REQUEST_ID_PREFIX') ? { requestIdPrefix: optional('NAV_REQUEST_ID_PREFIX') as string } : {}),
    navTimeoutMs: number('NAV_TIMEOUT_MS', 30_000),
    pollTimeoutMs: number('NAV_POLL_TIMEOUT_MS', 60_000),
    pollInitialDelayMs: number('NAV_POLL_INITIAL_DELAY_MS', 1_000),
    pdfLanguage: pdfLanguageRaw,
    proformaTtlMs: number('PROFORMA_TTL_SECONDS', 3_600) * 1_000,
    proformaCacheMax: number('PROFORMA_CACHE_MAX', 200),
    bodyLimit: optional('BODY_LIMIT') ?? '1mb',
  };
}

export { MOCK_CREDENTIALS };
