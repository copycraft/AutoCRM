'use client';

// The newsletter tag lists down the side of the Marketing page, like the MiniCRM sidebar:
// click a tag to list its subscribers. "Szerkesztés" turns the same lists into an editor:
// type and Enter to add, click a name to rename, the swatch to recolour, arrows to
// reorder, and a new heading starts a new section.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, ArrowDown, ArrowUp, Pencil, RotateCcw } from 'lucide-react';
import { newsletterApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { cn } from '@/lib/utils/format';
import { IconButton, PALETTE, SwatchButton } from '@/components/tags/TagKit';
import { newsletterTagsKey, sections } from './NewsletterTags';
import type { NewsletterTag } from '@/lib/api/types';

export type Selection = { kind: 'all' } | { kind: 'untagged' } | { kind: 'tag'; id: number };

export function TagSidebar({
  tags,
  counts,
  selection,
  onSelect,
  editable,
}: {
  tags: NewsletterTag[];
  counts: { active: number; total: number; untagged: number } | undefined;
  selection: Selection;
  onSelect: (s: Selection) => void;
  editable: boolean;
}) {
  const t = useTranslations('marketing');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newSection, setNewSection] = useState('');
  const [newSectionTag, setNewSectionTag] = useState('');
  const archived = useQuery({
    queryKey: newsletterTagsKey(true),
    queryFn: () => newsletterApi.tags(true),
    enabled: editing,
  });

  const refresh = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['newsletter-tags'] });
    void qc.invalidateQueries({ queryKey: ['newsletter-subscribers'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const update = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof newsletterApi.updateTag>[1] }) =>
      newsletterApi.updateTag(id, body),
    onSuccess: refresh,
    onError,
  });
  const create = useMutation({
    mutationFn: (body: { section: string; label: string }) => {
      const used = new Set(tags.filter((x) => x.section === body.section).map((x) => x.color));
      return newsletterApi.createTag({ ...body, color: PALETTE.find((c) => !used.has(c)) ?? '#dbe8ff' });
    },
    onSuccess: refresh,
    onError,
  });
  const reorder = useMutation({
    mutationFn: ({ section, ids }: { section: string; ids: number[] }) => newsletterApi.reorderTags(section, ids),
    onSuccess: refresh,
    onError,
  });

  const groups = sections(tags);
  const isSel = (s: Selection) =>
    s.kind === selection.kind && (s.kind !== 'tag' || (selection.kind === 'tag' && s.id === selection.id));

  return (
    <nav className="space-y-5 text-body" aria-label={t('tagsTitle')}>
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-section font-semibold">{t('tagsTitle')}</h2>
        {editable && (
          <button
            type="button"
            className={cn('btn-sm', editing ? 'btn-primary' : 'btn-ghost')}
            onClick={() => setEditing((e) => !e)}
          >
            <Pencil className="h-3.5 w-3.5" aria-hidden />
            {editing ? t('editDone') : t('edit')}
          </button>
        )}
      </div>
      {error && (
        <p className="rounded-lg bg-signal/10 px-3 py-2 text-metadata text-signal" role="alert">
          {error}
        </p>
      )}

      <ul className="space-y-0.5">
        <SideItem active={isSel({ kind: 'all' })} onClick={() => onSelect({ kind: 'all' })} label={t('everyone')}
          count={counts ? `${counts.active}/${counts.total}` : undefined} />
        <SideItem active={isSel({ kind: 'untagged' })} onClick={() => onSelect({ kind: 'untagged' })} label={t('untagged')}
          count={counts ? String(counts.untagged) : undefined} />
      </ul>

      {groups.map((g) => (
        <section key={g.section} className="border-t border-steel-200 pt-4">
          <h3 className="mb-2 font-semibold text-steel-900">{g.section}</h3>
          <ul className="space-y-0.5">
            {g.tags.map((tag, i) =>
              editing ? (
                <EditRow
                  key={tag.id}
                  tag={tag}
                  first={i === 0}
                  last={i === g.tags.length - 1}
                  onUpdate={(body) => update.mutate({ id: tag.id, body })}
                  onMove={(by) => {
                    const ids = g.tags.map((x) => x.id);
                    const [id] = ids.splice(i, 1);
                    if (id !== undefined) ids.splice(i + by, 0, id);
                    reorder.mutate({ section: g.section, ids });
                  }}
                />
              ) : (
                <li key={tag.id} className="flex items-center gap-2">
                  <span className="h-4 w-4 shrink-0 rounded ring-1 ring-inset ring-black/10" style={{ backgroundColor: tag.color }} />
                  <button
                    type="button"
                    className={cn(
                      'min-w-0 flex-1 truncate rounded px-1 py-0.5 text-left hover:bg-steel-200/40',
                      isSel({ kind: 'tag', id: tag.id }) && 'bg-steel-200/60 font-semibold',
                    )}
                    title={tag.label}
                    onClick={() => onSelect({ kind: 'tag', id: tag.id })}
                  >
                    {tag.label}{' '}
                    <span className="font-mono text-metadata text-steel-500">
                      ({tag.active_subscribers}/{tag.total_subscribers})
                    </span>
                  </button>
                </li>
              ),
            )}
          </ul>
          {editing && (
            <form
              className="mt-2"
              onSubmit={(e) => {
                e.preventDefault();
                const input = e.currentTarget.elements.namedItem('label') as HTMLInputElement;
                const label = input.value.trim();
                if (label) create.mutate({ section: g.section, label }, { onSuccess: () => (input.value = '') });
              }}
            >
              <input name="label" className="input h-8" placeholder={t('newTag')} />
            </form>
          )}
        </section>
      ))}

      {editing && (
        <form
          className="space-y-2 border-t border-steel-200 pt-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (!newSection.trim() || !newSectionTag.trim()) return;
            create.mutate(
              { section: newSection.trim(), label: newSectionTag.trim() },
              {
                onSuccess: () => {
                  setNewSection('');
                  setNewSectionTag('');
                },
              },
            );
          }}
        >
          <h3 className="font-semibold">{t('newSection')}</h3>
          <input className="input h-8" placeholder={t('newSectionName')} value={newSection} onChange={(e) => setNewSection(e.target.value)} />
          <input className="input h-8" placeholder={t('newSectionFirstTag')} value={newSectionTag} onChange={(e) => setNewSectionTag(e.target.value)} />
          <button className="btn-secondary btn-sm" type="submit" disabled={!newSection.trim() || !newSectionTag.trim()}>
            {t('add')}
          </button>
        </form>
      )}

      {editing && (archived.data?.items.length ?? 0) > 0 && (
        <details className="border-t border-steel-200 pt-4">
          <summary className="cursor-pointer font-semibold">
            {t('archived', { count: archived.data?.items.length ?? 0 })}
          </summary>
          <ul className="mt-2 space-y-1">
            {archived.data?.items.map((tag) => (
              <li key={tag.id} className="flex items-center gap-2">
                <span className="h-3 w-3 rounded-sm" style={{ backgroundColor: tag.color }} />
                <span className="min-w-0 flex-1 truncate">{tag.section} · {tag.label}</span>
                <IconButton label={t('unarchive')} onClick={() => update.mutate({ id: tag.id, body: { archived: false } })}>
                  <RotateCcw className="h-3.5 w-3.5" aria-hidden />
                </IconButton>
              </li>
            ))}
          </ul>
        </details>
      )}
    </nav>
  );
}

function SideItem({ label, count, active, onClick }: { label: string; count?: string; active: boolean; onClick: () => void }) {
  return (
    <li>
      <button
        type="button"
        className={cn('w-full rounded px-1 py-0.5 text-left hover:bg-steel-200/40', active && 'bg-steel-200/60 font-semibold')}
        onClick={onClick}
      >
        {label} {count && <span className="font-mono text-metadata text-steel-500">({count})</span>}
      </button>
    </li>
  );
}

function EditRow({
  tag,
  first,
  last,
  onUpdate,
  onMove,
}: {
  tag: NewsletterTag;
  first: boolean;
  last: boolean;
  onUpdate: (body: Parameters<typeof newsletterApi.updateTag>[1]) => void;
  onMove: (by: -1 | 1) => void;
}) {
  const t = useTranslations('marketing');
  const [label, setLabel] = useState<string | null>(null);
  const save = () => {
    const next = label?.trim();
    setLabel(null);
    if (next && next !== tag.label) onUpdate({ label: next });
  };
  return (
    <li className="flex items-center gap-1.5">
      <SwatchButton color={tag.color} editable label={t('color')} onPick={(c) => onUpdate({ color: c })} />
      {label !== null ? (
        <input
          autoFocus
          className="input h-7 min-w-0 flex-1 py-0"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          onBlur={save}
          onKeyDown={(e) => {
            if (e.key === 'Enter') save();
            if (e.key === 'Escape') setLabel(null);
          }}
        />
      ) : (
        <button type="button" className="min-w-0 flex-1 truncate text-left hover:underline" title={t('rename')} onClick={() => setLabel(tag.label)}>
          {tag.label}
        </button>
      )}
      <IconButton label={t('moveUp')} disabled={first} onClick={() => onMove(-1)}>
        <ArrowUp className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
      <IconButton label={t('moveDown')} disabled={last} onClick={() => onMove(1)}>
        <ArrowDown className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
      <IconButton label={t('archive')} onClick={() => onUpdate({ archived: true })}>
        <Archive className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
    </li>
  );
}
