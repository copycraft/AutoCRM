import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Route smoke tests (V0.2). These exist because `tsc`, `lint` and `build` all
// passed on a screen that printed `editing ? (` to the user: valid JSX, wrong
// meaning. Nothing but rendering catches that class of defect.
export default defineConfig({
  plugins: [react()],
  resolve: {
    // Mirrors the single `@/*` path mapping in tsconfig.json.
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    css: false,
  },
});
