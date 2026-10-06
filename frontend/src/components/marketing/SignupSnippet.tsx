'use client';

// The website form for one newsletter list: a ready-to-paste snippet that posts the
// reader's address with this list's id. Double opt-in still applies — the reader joins
// the list when they confirm from the letter.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { Check, Copy } from 'lucide-react';

/** The fetch call the website's own backend makes; the key never goes to the browser. */
export function signupSnippet(apiBase: string, tagId: number): string {
  return [
    `// A weboldal szerveréről (a kulcs nem kerülhet a böngészőbe):`,
    `await fetch('${apiBase}/api/newsletter/subscribe', {`,
    `  method: 'POST',`,
    `  headers: { 'Content-Type': 'application/json', 'X-Newsletter-Key': process.env.NEWSLETTER_API_KEY },`,
    `  body: JSON.stringify({ email, name, tag_ids: [${tagId}] }),`,
    `});`,
  ].join('\n');
}

export function SignupSnippet({ tagId, tagLabel }: { tagId: number; tagLabel: string }) {
  const t = useTranslations('marketing');
  const [copied, setCopied] = useState(false);
  const base = typeof window === 'undefined' ? '' : window.location.origin;
  const code = signupSnippet(base, tagId);
  return (
    <div className="mt-4 border-t border-steel-200 pt-4" data-testid="signup-snippet">
      <h3 className="text-body font-semibold">{t('signupTitle', { tag: tagLabel })}</h3>
      <p className="mb-2 text-metadata text-steel-500">{t('signupHint')}</p>
      <pre className="max-h-48 overflow-auto rounded-lg bg-steel-900 p-2 font-mono text-[11px] leading-snug text-white">{code}</pre>
      <button
        type="button"
        className="btn-ghost btn-sm mt-2"
        onClick={() => {
          void navigator.clipboard?.writeText(code).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          });
        }}
      >
        {copied ? <Check className="h-4 w-4" aria-hidden /> : <Copy className="h-4 w-4" aria-hidden />}
        {copied ? t('copied') : t('copy')}
      </button>
    </div>
  );
}
