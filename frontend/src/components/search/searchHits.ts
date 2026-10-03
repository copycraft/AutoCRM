// Turns the search API's groups into one flat, keyboard-navigable list plus the group
// headings, so the sidebar box and the command palette show the same things the same way.

import type { components } from '@/lib/api/schema.gen';

type Results = components['schemas']['GlobalResults'];

export interface SearchHit {
  href: string;
  title: string;
  sub: string;
}

export interface SearchGroup {
  label: string;
  from: number;
  to: number;
}

export interface GroupLabels {
  orders: string;
  partners: string;
  leads: string;
  contacts: string;
  emails: string;
  employees: string;
}

const join = (parts: (string | null | undefined)[]) => parts.filter(Boolean).join(' · ');

export function buildSearchHits(
  data: Results | undefined,
  locale: string,
  labels: GroupLabels,
): { flat: SearchHit[]; groups: SearchGroup[] } {
  if (!data) return { flat: [], groups: [] };
  const lists: [string, SearchHit[]][] = [
    [
      labels.orders,
      data.orders.map((o) => ({
        href: `/${locale}/orders/${o.id}`,
        title: `#${o.number} · ${o.plate ?? '—'}`,
        sub: `${o.title} · ${o.stage_label}`,
      })),
    ],
    [
      labels.partners,
      data.partners.map((p) => ({
        href: `/${locale}/partners/${p.id}`,
        title: p.name,
        sub: join([p.kind === 'business' ? 'Üzleti' : 'Magán', p.city]),
      })),
    ],
    [
      labels.leads,
      data.leads.map((l) => ({
        href: `/${locale}/leads/${l.id}`,
        title: l.title,
        sub: join([l.contact_name, l.stage_label]),
      })),
    ],
    [
      labels.contacts,
      data.contacts.map((c) => ({
        // A contact has no page of its own: it opens the company it works for.
        href: `/${locale}/partners/${c.partner_id}`,
        title: c.name,
        sub: join([c.partner_name, c.phone, c.email]),
      })),
    ],
    [
      labels.emails,
      data.emails.map((e) => ({
        href: `/${locale}/emails/${e.id}`,
        title: e.subject,
        sub: e.to_address,
      })),
    ],
    [
      labels.employees,
      data.employees.map((e) => ({
        // The HR page opens with the name already searched.
        href: `/${locale}/hr?q=${encodeURIComponent(e.full_name)}`,
        title: e.full_name,
        sub: join([e.company_phone, e.email, e.archived ? 'Kilépett' : null]),
      })),
    ],
  ];
  const flat: SearchHit[] = [];
  const groups: SearchGroup[] = [];
  for (const [label, hits] of lists) {
    if (hits.length === 0) continue;
    groups.push({ label, from: flat.length, to: flat.length + hits.length });
    flat.push(...hits);
  }
  return { flat, groups };
}
