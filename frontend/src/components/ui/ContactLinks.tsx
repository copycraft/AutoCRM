import type { ReactNode } from 'react';

/** Strips everything a `tel:` URI chokes on, keeping a leading `+`. Display keeps the original. */
function telHref(phone: string): string {
  const trimmed = phone.trim();
  const plus = trimmed.startsWith('+') ? '+' : '';
  return `tel:${plus}${trimmed.replace(/[^\d]/g, '')}`;
}

export function PhoneValue({ value, mono }: { value: string | null | undefined; mono?: boolean }) {
  if (!value?.trim()) return <>—</>;
  return (
    <a href={telHref(value)} className={mono ? 'font-mono' : undefined}>
      {value}
    </a>
  );
}

export function EmailValue({ value, mono }: { value: string | null | undefined; mono?: boolean }) {
  if (!value?.trim()) return <>—</>;
  return (
    <a href={`mailto:${value.trim()}`} className={mono ? 'font-mono' : undefined}>
      {value}
    </a>
  );
}

/** Position · email · phone with the last two tappable, for contact rows. */
export function ContactLine({
  position,
  email,
  phone,
  className,
}: {
  position?: string | null;
  email?: string | null;
  phone?: string | null;
  className?: string;
}): ReactNode {
  const parts: ReactNode[] = [];
  if (position?.trim()) parts.push(<span key="p">{position}</span>);
  if (email?.trim()) parts.push(<EmailValue key="e" value={email} />);
  if (phone?.trim()) parts.push(<PhoneValue key="t" value={phone} />);
  if (parts.length === 0) return <>—</>;
  return (
    <span className={className}>
      {parts.map((part, i) => (
        <span key={i}>
          {i > 0 && ' · '}
          {part}
        </span>
      ))}
    </span>
  );
}
