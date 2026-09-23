import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { fileURLToPath } from 'node:url';
import { createApp } from './app.js';
import { loadConfig } from './config.js';
import type { Config } from './config.js';
import { createNavContext } from './nav.js';
import type { MockServer } from '@open-nav/mock-server';

export interface RunningSidecar {
  /** Where it is listening, e.g. `http://127.0.0.1:8080`. */
  url: string;
  port: number;
  config: Config;
  /** The in-process fake NAV, when MOCK_MODE is on. Tests assert on its state. */
  mock?: MockServer;
  close(): Promise<void>;
}

/** Start the sidecar. Exported so tests can run it on an ephemeral port. */
export async function startSidecar(config: Config = loadConfig()): Promise<RunningSidecar> {
  const context = await createNavContext(config);
  const server = createServer(createApp(context));

  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(config.port, config.host, () => {
      server.removeListener('error', reject);
      resolve();
    });
  });

  const address = server.address() as AddressInfo;
  const host = address.address === '::' || address.address === '0.0.0.0' ? '127.0.0.1' : address.address;
  const url = `http://${host.includes(':') ? `[${host}]` : host}:${address.port}`;

  return {
    url,
    port: address.port,
    config,
    ...(context.mock ? { mock: context.mock } : {}),
    async close() {
      await new Promise<void>((resolve) => server.close(() => resolve()));
      // Ends keep-alive connections the close above only stops accepting on.
      server.closeAllConnections?.();
      await context.close();
    },
  };
}

async function main(): Promise<void> {
  const config = loadConfig();
  const sidecar = await startSidecar(config);
  console.log(
    `[nav-sidecar] listening on ${sidecar.url} — NAV environment: ` +
      `${config.mockMode ? 'mock' : config.environment}, softwareId: ${config.software.softwareId}`,
  );

  // A container stops with SIGTERM; finish in-flight requests rather than
  // cutting a submission's polling loop off mid-transaction.
  let stopping = false;
  const stop = (signal: string) => {
    if (stopping) return;
    stopping = true;
    console.log(`[nav-sidecar] ${signal} received, shutting down`);
    void sidecar.close().then(
      () => process.exit(0),
      (error: unknown) => {
        console.error('[nav-sidecar] shutdown failed:', error);
        process.exit(1);
      },
    );
  };
  process.on('SIGTERM', () => stop('SIGTERM'));
  process.on('SIGINT', () => stop('SIGINT'));
}

// Only when run as a program; importing this module starts nothing.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().catch((error: unknown) => {
    console.error('[nav-sidecar] failed to start:', error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
