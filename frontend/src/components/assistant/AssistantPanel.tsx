'use client';

// The assistant: a chat panel on every page. A small local model answers questions about
// the CRM through read-only tools and builds list filters; each filter it makes appears as
// a card to open the list with, or to save as a view of that list. It never changes data.
// Hidden entirely when no model is configured.

import { useEffect, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Bot, Filter, Save, Send, Sparkles, Trash2, Wrench, X } from 'lucide-react';
import { assistantApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { saveViewTo } from '@/hooks/useSavedViews';
import { useToast } from '@/components/ui/Toasts';
import { cn } from '@/lib/utils/format';
import type { AssistantReply, FilterSuggestion } from '@/lib/api/types';

interface Turn {
  role: 'user' | 'assistant';
  content: string;
  filters?: FilterSuggestion[];
  tools?: string[];
  error?: boolean;
}

const STORE = 'autocrm:assistant';

/** The saved-view key each filter list belongs to (see SavedViewsBar). */
const VIEW_KEY: Record<string, string> = {
  leads: 'leads',
  orders: 'orders',
  partners: 'partners',
  incoming_invoices: 'incoming_invoices',
  subscribers: 'subscribers',
};

export function AssistantPanel() {
  const t = useTranslations('assistant');
  const status = useQuery({ queryKey: ['assistant'], queryFn: () => assistantApi.status(), staleTime: 5 * 60_000, retry: false });
  const [open, setOpen] = useState(false);
  if (!status.data?.enabled) return null;
  return (
    <>
      {!open && (
        <button
          type="button"
          className="fixed bottom-5 right-5 z-40 flex items-center gap-2 rounded-full bg-steel-900 px-4 py-3 text-body font-medium text-white shadow-lg hover:bg-steel-900/90"
          onClick={() => setOpen(true)}
          aria-label={t('title')}
        >
          <Sparkles className="h-5 w-5" aria-hidden />
          <span className="hidden sm:inline">{t('title')}</span>
        </button>
      )}
      {open && <Chat model={status.data.model ?? null} onClose={() => setOpen(false)} />}
    </>
  );
}

function Chat({ model, onClose }: { model: string | null; onClose: () => void }) {
  const t = useTranslations('assistant');
  const ter = useTranslations('errors');
  const [turns, setTurns] = useState<Turn[]>(() => {
    try {
      const raw = window.sessionStorage.getItem(STORE);
      const parsed: unknown = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? (parsed as Turn[]).slice(-40) : [];
    } catch {
      return [];
    }
  });
  const [input, setInput] = useState('');
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    try {
      window.sessionStorage.setItem(STORE, JSON.stringify(turns.slice(-40)));
    } catch {
      /* best-effort */
    }
    bottom.current?.scrollIntoView({ block: 'end' });
  }, [turns]);

  const ask = useMutation({
    mutationFn: (history: Turn[]) =>
      assistantApi.chat(
        history.filter((x) => !x.error).map((x) => ({ role: x.role, content: x.content })),
      ),
    onSuccess: (r: AssistantReply) =>
      setTurns((ts) => [
        ...ts,
        { role: 'assistant', content: r.reply, filters: r.filters, tools: r.steps.map((s) => s.tool) },
      ]),
    onError: (e) =>
      setTurns((ts) => [...ts, { role: 'assistant', content: errorMessage(e, ter, ter('unknownError')), error: true }]),
  });

  const send = (text: string) => {
    const q = text.trim();
    if (!q || ask.isPending) return;
    const next = [...turns, { role: 'user' as const, content: q }];
    setTurns(next);
    setInput('');
    ask.mutate(next);
  };

  const examples = [t('example1'), t('example2'), t('example3'), t('example4')];

  return (
    <aside
      className="fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col border-l border-steel-200 bg-surface shadow-2xl"
      role="dialog"
      aria-label={t('title')}
      onKeyDown={(e) => e.key === 'Escape' && onClose()}
    >
      <header className="flex items-center gap-2 border-b border-steel-200 px-4 py-3">
        <Bot className="h-5 w-5 text-steel-500" aria-hidden />
        <div className="min-w-0 flex-1">
          <p className="font-semibold">{t('title')}</p>
          <p className="truncate text-metadata text-steel-500">{t('subtitle', { model: model ?? '—' })}</p>
        </div>
        {turns.length > 0 && (
          <button type="button" className="btn-ghost btn-sm" onClick={() => setTurns([])} aria-label={t('clear')} title={t('clear')}>
            <Trash2 className="h-4 w-4" aria-hidden />
          </button>
        )}
        <button type="button" className="btn-ghost btn-sm" onClick={onClose} aria-label={t('close')}>
          <X className="h-4 w-4" aria-hidden />
        </button>
      </header>

      <div className="flex-1 space-y-3 overflow-y-auto px-4 py-4">
        {turns.length === 0 && (
          <div className="space-y-3">
            <p className="text-body text-steel-500">{t('intro')}</p>
            <div className="flex flex-col gap-2">
              {examples.map((ex) => (
                <button key={ex} type="button" className="rounded-lg border border-steel-200 px-3 py-2 text-left text-body hover:bg-steel-200/40" onClick={() => send(ex)}>
                  {ex}
                </button>
              ))}
            </div>
          </div>
        )}
        {turns.map((turn, i) => (
          <TurnView key={i} turn={turn} onNavigate={onClose} />
        ))}
        {ask.isPending && (
          <p className="flex items-center gap-2 text-metadata text-steel-500" role="status">
            <span className="h-2 w-2 animate-pulse rounded-full bg-cold" />
            {t('thinking')}
          </p>
        )}
        <div ref={bottom} />
      </div>

      <form
        className="flex gap-2 border-t border-steel-200 p-3"
        onSubmit={(e) => {
          e.preventDefault();
          send(input);
        }}
      >
        <input
          autoFocus
          className="input flex-1"
          placeholder={t('placeholder')}
          value={input}
          maxLength={2000}
          onChange={(e) => setInput(e.target.value)}
          aria-label={t('placeholder')}
        />
        <button type="submit" className="btn-primary btn-sm" disabled={!input.trim() || ask.isPending} aria-label={t('send')}>
          <Send className="h-4 w-4" aria-hidden />
        </button>
      </form>
      <p className="px-3 pb-2 text-[11px] text-steel-500">{t('readOnly')}</p>
    </aside>
  );
}

function TurnView({ turn, onNavigate }: { turn: Turn; onNavigate: () => void }) {
  const t = useTranslations('assistant');
  if (turn.role === 'user') {
    return (
      <div className="ml-10 rounded-2xl rounded-br-sm bg-steel-900 px-3 py-2 text-body text-white whitespace-pre-wrap">
        {turn.content}
      </div>
    );
  }
  return (
    <div className="mr-6 space-y-2">
      <div
        className={cn(
          'rounded-2xl rounded-bl-sm px-3 py-2 text-body whitespace-pre-wrap',
          turn.error ? 'bg-signal/10 text-signal' : 'bg-panel text-steel-900',
        )}
      >
        {turn.content}
      </div>
      {turn.tools && turn.tools.length > 0 && (
        <p className="flex flex-wrap items-center gap-1 text-[11px] text-steel-500">
          <Wrench className="h-3 w-3" aria-hidden />
          {turn.tools.map((name, i) => (
            <span key={i} className="rounded bg-steel-200/60 px-1 font-mono">
              {name}
            </span>
          ))}
        </p>
      )}
      {turn.filters?.map((f, i) => <FilterCard key={i} filter={f} onNavigate={onNavigate} />)}
    </div>
  );
}

function FilterCard({ filter, onNavigate }: { filter: FilterSuggestion; onNavigate: () => void }) {
  const t = useTranslations('assistant');
  const locale = useLocale();
  const router = useRouter();
  const toast = useToast();
  const viewKey = VIEW_KEY[filter.list];
  return (
    <div className="rounded-lg border border-cold/40 bg-cold/5 p-3">
      <p className="flex items-center gap-2 text-body font-medium">
        <Filter className="h-4 w-4 text-cold" aria-hidden />
        {filter.label}
      </p>
      <div className="mt-2 flex flex-wrap gap-2">
        <button
          type="button"
          className="btn-primary btn-sm"
          onClick={() => {
            router.push(`/${locale}${filter.path}`);
            onNavigate();
          }}
        >
          {t('openFilter')}
        </button>
        {viewKey && (
          <button
            type="button"
            className="btn-secondary btn-sm"
            onClick={() => {
              saveViewTo(viewKey, filter.label, filter.params);
              toast.success(t('savedView'));
            }}
          >
            <Save className="h-3.5 w-3.5" aria-hidden />
            {t('saveView')}
          </button>
        )}
      </div>
    </div>
  );
}
