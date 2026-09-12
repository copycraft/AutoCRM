// `next-intl/server` needs a request scope that jsdom has no way to provide.
// The server pages only ever call getTranslations/getMessages, so resolving
// straight out of the message catalogue is the whole contract.
import messages from '@/messages/hu.json';

type Catalogue = Record<string, Record<string, unknown>>;

export async function getTranslations({
  namespace,
}: {
  locale?: string;
  namespace?: string;
}): Promise<(key: string) => string> {
  const table = namespace ? ((messages as unknown as Catalogue)[namespace] ?? {}) : {};
  return (key: string) => String(table[key] ?? `${namespace ?? ''}.${key}`);
}

export async function getMessages(): Promise<unknown> {
  return messages;
}

export async function getLocale(): Promise<string> {
  return 'hu';
}

export function getRequestConfig(fn: unknown): unknown {
  return fn;
}
