'use client';

import { useState } from 'react';
import { Check, Copy, Link2 } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useToast } from '@/components/ui/Toasts';

async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Clipboard API unavailable (permissions, insecure context): fall back to selection.
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    try {
      document.execCommand('copy');
    } catch {
      /* best-effort */
    }
    ta.remove();
  }
}

export function CopyButton({ value, label }: { value: string | null | undefined; label: string }) {
  const t = useTranslations('qol');
  const toast = useToast();
  const [copied, setCopied] = useState(false);
  const text = (value ?? '').trim();
  if (!text) return null;

  const onCopy = async () => {
    await copyText(text);
    setCopied(true);
    toast.success(t('copied', { label }));
    window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      type="button"
      onClick={() => void onCopy()}
      title={t('copyLabel', { label })}
      aria-label={t('copyLabel', { label })}
      className="inline-flex items-center rounded border border-steel-200 p-1 text-steel-500 transition-colors hover:border-steel-900 hover:text-steel-900"
    >
      {copied ? <Check className="h-3.5 w-3.5" aria-hidden /> : <Copy className="h-3.5 w-3.5" aria-hidden />}
    </button>
  );
}

/** Copies the current page URL — detail pages stay shareable as filters move to the URL. */
export function CopyLinkButton() {
  const t = useTranslations('qol');
  const toast = useToast();
  const [copied, setCopied] = useState(false);

  const onCopy = async () => {
    await copyText(window.location.href);
    setCopied(true);
    toast.success(t('linkCopied'));
    window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      type="button"
      onClick={() => void onCopy()}
      title={t('copyLink')}
      aria-label={t('copyLink')}
      className="inline-flex items-center rounded border border-steel-200 p-1 text-steel-500 transition-colors hover:border-steel-900 hover:text-steel-900"
    >
      {copied ? <Check className="h-3.5 w-3.5" aria-hidden /> : <Link2 className="h-3.5 w-3.5" aria-hidden />}
    </button>
  );
}
