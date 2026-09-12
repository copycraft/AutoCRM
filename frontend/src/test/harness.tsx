import { NextIntlClientProvider } from 'next-intl';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, type RenderResult } from '@testing-library/react';
import { expect } from 'vitest';
import type { ReactElement, ReactNode } from 'react';
import { AuthProvider } from '@/lib/auth/context';
import messages from '@/messages/hu.json';

function testClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  });
}

function Providers({ children }: { children: ReactNode }) {
  return (
    <NextIntlClientProvider locale="hu" messages={messages} timeZone="Europe/Budapest">
      <QueryClientProvider client={testClient()}>
        <AuthProvider>{children}</AuthProvider>
      </QueryClientProvider>
    </NextIntlClientProvider>
  );
}

export function renderPage(ui: ReactElement): RenderResult {
  return render(<Providers>{ui}</Providers>);
}

/**
 * Source text that reached the user (V0.2).
 *
 * An unbraced ternary inside a JSX element — `{'cond ? ('}` — is valid JSX, type
 * checks, lints and builds; it renders the source as body text and both branches
 * at once. These are the shapes that produces.
 */
const SOURCE_TEXT: RegExp[] = [
  /\?\s*\($/, //          `editing ? (`
  /^\s*\)\s*:\s*\($/, //  `) : (`
];

function textNodes(root: Node): string[] {
  const out: string[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    const value = (node.nodeValue ?? '').trim();
    if (value) out.push(value);
    node = walker.nextNode();
  }
  return out;
}

/** Fails if any rendered text node looks like unevaluated JSX source. */
export function expectNoSourceText(container: HTMLElement): void {
  for (const value of textNodes(container)) {
    for (const pattern of SOURCE_TEXT) {
      expect(
        pattern.test(value),
        `rendered source text ${JSON.stringify(value)} matches ${String(pattern)}`,
      ).toBe(false);
    }
  }
}
