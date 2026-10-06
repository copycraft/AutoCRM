'use client';

// Newsletter tags as the rest of the app uses them: the live list, its sections in
// order, and the picker for putting tags on a subscriber or aiming a blast.

import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { newsletterApi } from '@/lib/api/endpoints';
import { Chip, GroupedTagPicker } from '@/components/tags/TagKit';
import type { NewsletterTag } from '@/lib/api/types';

export const newsletterTagsKey = (archived = false) => ['newsletter-tags', archived] as const;

export function useNewsletterTags() {
  return useQuery({ queryKey: newsletterTagsKey(), queryFn: () => newsletterApi.tags() });
}

/** Sections in list order (where each one's first tag is), each with its tags. */
export function sections(tags: NewsletterTag[]): { section: string; tags: NewsletterTag[] }[] {
  const out: { section: string; tags: NewsletterTag[] }[] = [];
  for (const tag of [...tags].sort((a, b) => a.position - b.position || a.id - b.id)) {
    const key = tag.section.toLowerCase();
    const group = out.find((g) => g.section.toLowerCase() === key);
    if (group) group.tags.push(tag);
    else out.push({ section: tag.section, tags: [tag] });
  }
  return out;
}

export function NewsletterTagPicker({
  value,
  onChange,
  label,
  disabled,
  align,
}: {
  value: number[];
  onChange: (ids: number[]) => void;
  label: string;
  disabled?: boolean;
  align?: 'left' | 'right';
}) {
  const tags = useNewsletterTags();
  const groups = useMemo(
    () =>
      sections(tags.data?.items ?? []).map((g) => ({
        title: g.section,
        tags: g.tags.map((x) => ({ id: x.id, label: x.label, color: x.color })),
      })),
    [tags.data],
  );
  return (
    <GroupedTagPicker
      groups={groups}
      value={value}
      onChange={onChange}
      label={label}
      disabled={disabled}
      align={align}
    />
  );
}

/** The chips for a set of tag ids; ids of archived tags are shown from `all` if present. */
export function NewsletterChips({
  ids,
  all,
  onClick,
  onRemove,
}: {
  ids: number[];
  all: NewsletterTag[];
  onClick?: (id: number) => void;
  onRemove?: (id: number) => void;
}) {
  return (
    <>
      {ids.map((id) => {
        const tag = all.find((x) => x.id === id);
        return (
          <Chip
            key={id}
            label={tag?.label ?? `#${id}`}
            color={tag?.color ?? '#dde1e6'}
            title={tag?.section}
            onClick={onClick ? () => onClick(id) : undefined}
            onRemove={onRemove ? () => onRemove(id) : undefined}
          />
        );
      })}
    </>
  );
}
