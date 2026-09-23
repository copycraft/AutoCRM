import Link from 'next/link';
import { Fragment, useEffect, useState } from 'react';
import { useTranslations } from 'next-intl';
import { lastListUrl } from '@/hooks/useListMemory';

export interface Crumb {
  href?: string;
  label: string;
}

export function Breadcrumbs({ items }: { items: Crumb[] }) {
  return (
    <nav aria-label="Morzsamenü" className="flex flex-wrap items-center gap-1.5 text-metadata text-steel-500">
      {items.map((c, i) => (
        <Fragment key={`${c.label}-${i}`}>
          {i > 0 && (
            <span aria-hidden className="select-none">
              /
            </span>
          )}
          {c.href && i < items.length - 1 ? (
            <Link href={c.href} className="underline hover:text-steel-900">
              {c.label}
            </Link>
          ) : (
            <span aria-current={i === items.length - 1 ? 'page' : undefined} className="text-steel-900">
              {c.label}
            </span>
          )}
        </Fragment>
      ))}
    </nav>
  );
}

/** "← Vissza a listához" — resolves to the last visited filtered list, else the fallback. */
export function BackToList({ listKey, fallbackHref }: { listKey: string; fallbackHref: string }) {
  const t = useTranslations('qol');
  const [href, setHref] = useState(fallbackHref);
  useEffect(() => {
    setHref(lastListUrl(listKey, fallbackHref));
  }, [listKey, fallbackHref]);
  return (
    <Link href={href} className="text-body text-steel-500 underline hover:text-steel-900">
      ← {t('backToList')}
    </Link>
  );
}
