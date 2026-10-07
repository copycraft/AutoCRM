'use client';

// The whole correspondence with a lead's customer, both ways, as one thread (0049): our
// letters (hand-written and automatic) and their replies read back from the mailbox.
// Answering one opens the composer threaded under it, so it lands in the same
// conversation in the customer's mail app.

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { ArrowDownLeft, ArrowUpRight, Reply } from 'lucide-react';
import { conversationApi } from '@/lib/api/endpoints';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { cn } from '@/lib/utils/format';
import type { components } from '@/lib/api/schema.gen';

type Item = components['schemas']['ConversationItem'];

/** The reply link: subject with one "Re:", the letter quoted (capped: it travels in the URL). */
function replyHref(locale: string, leadId: number, item: Item): string {
  const subject = /^(re|vá|va):/i.test(item.subject.trim()) ? item.subject.trim() : `Re: ${item.subject.trim()}`;
  const params = new URLSearchParams({
    lead_id: String(leadId),
    to: item.from_address,
    reply_to: String(item.id),
    subject,
    quote: item.body_text.slice(0, 1500),
  });
  return `/${locale}/emails/new?${params.toString()}`;
}

function Letter({ item, leadId, canReply }: { item: Item; leadId: number; canReply: boolean }) {
  const t = useTranslations('conversation');
  const locale = useLocale();
  const [open, setOpen] = useState(false);
  const incoming = item.direction === 'in';
  const text = item.body_text.trim();
  const long = text.length > 400;
  return (
    <li className={cn('flex', incoming ? 'justify-start' : 'justify-end')}>
      <div
        className={cn(
          'max-w-[85%] rounded-xl border px-4 py-3',
          incoming ? 'border-steel-200 bg-surface' : 'border-cold/30 bg-cold/5',
        )}
      >
        <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-metadata text-steel-500">
          {incoming ? (
            <ArrowDownLeft className="h-3.5 w-3.5" aria-label={t('in')} />
          ) : (
            <ArrowUpRight className="h-3.5 w-3.5" aria-label={t('out')} />
          )}
          <span className="font-medium text-steel-800">
            {incoming ? item.from_name || item.from_address : item.sent_by_name || t('automatic')}
          </span>
          {!incoming && item.to_address && <span>→ {item.to_address}</span>}
          <DateDisplay value={item.at} />
          {!incoming && item.status && item.status !== 'sent' && (
            <span className="badge-muted">{item.status}</span>
          )}
        </div>
        <p className="mt-1 text-body font-medium">{item.subject}</p>
        <p className="mt-1 whitespace-pre-wrap text-body text-steel-700">
          {open || !long ? text : `${text.slice(0, 400)}…`}
        </p>
        <div className="mt-2 flex flex-wrap gap-3 text-metadata">
          {long && (
            <button type="button" className="underline" onClick={() => setOpen((o) => !o)}>
              {open ? t('less') : t('more')}
            </button>
          )}
          {!incoming && (
            <Link className="underline" href={`/${locale}/emails/${item.id}`}>
              {t('openEmail')}
            </Link>
          )}
          {incoming && canReply && (
            <Link className="inline-flex items-center gap-1 font-medium underline" href={replyHref(locale, leadId, item)}>
              <Reply className="h-3.5 w-3.5" aria-hidden />
              {t('reply')}
            </Link>
          )}
        </div>
      </div>
    </li>
  );
}

export function Conversation({ leadId, canReply }: { leadId: number; canReply: boolean }) {
  const t = useTranslations('conversation');
  const q = useQuery({ queryKey: ['leads', leadId, 'conversation'], queryFn: () => conversationApi.forLead(leadId) });
  if (q.isLoading) return <LoadingState />;
  if (q.isError) return <ErrorState error={q.error} onRetry={() => void q.refetch()} />;
  const items = q.data?.items ?? [];
  if (items.length === 0) return <p className="text-body text-steel-500">{t('empty')}</p>;
  return (
    <ol className="space-y-3" aria-label={t('title')}>
      {items.map((item) => (
        <Letter key={`${item.direction}-${item.id}`} item={item} leadId={leadId} canReply={canReply} />
      ))}
    </ol>
  );
}
