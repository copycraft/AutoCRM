import { useLocale, useTranslations } from 'next-intl';

/**
 * Who the data controller is, as the page shows it. Every field is optional: a line whose
 * value is not configured is left out, so the public page never shows a placeholder.
 */
export interface PrivacyCompany {
  name?: string;
  address?: string;
  /** Company registration and/or tax number. */
  registration?: string;
  email?: string;
  phone?: string;
  /** Where the full Adatkezelési tájékoztató lives. */
  noticeUrl?: string;
}

interface Row {
  who: string;
  data: string;
  purpose: string;
  basis: string;
}

/**
 * The public "why am I here / who has my data" page: short, plain language, and linkable from
 * emails, website forms and the full privacy notice. It explains; the full notice is the
 * legal document.
 */
export function PrivacyContent({ company }: { company: PrivacyCompany }) {
  const t = useTranslations('privacy');
  const locale = useLocale();
  // Keyed objects in the messages file (its type does not allow arrays); shown in order.
  const rows = Object.values(t.raw('rows') as Record<string, Row>);
  const rights = Object.values(t.raw('rights') as Record<string, string>);

  const details: [string, string | undefined, 'mail' | 'tel' | undefined][] = [
    [t('controller'), company.name ?? 'Autotherm', undefined],
    [t('address'), company.address, undefined],
    [t('registration'), company.registration, undefined],
    [t('email'), company.email, 'mail'],
    [t('phone'), company.phone, 'tel'],
  ];

  return (
    <article className="mx-auto max-w-3xl space-y-8 px-4 py-10">
      <header>
        <p className="text-metadata text-steel-500">AUTOTHERM</p>
        <h1 className="mt-1 text-page-title font-semibold tracking-tight">{t('title')}</h1>
        <p className="mt-3 text-body">{t('intro')}</p>
      </header>

      <section aria-labelledby="privacy-how">
        <h2 id="privacy-how" className="text-section font-semibold">
          {t('howTitle')}
        </h2>
        <p className="mt-2 text-body">{t('howBody')}</p>
      </section>

      <section aria-labelledby="privacy-system">
        <h2 id="privacy-system" className="text-section font-semibold">
          {t('systemTitle')}
        </h2>
        <p className="mt-2 text-body">{t('systemBody')}</p>
      </section>

      <section aria-labelledby="privacy-who">
        <h2 id="privacy-who" className="text-section font-semibold">
          {t('whoTitle')}
        </h2>
        <p className="mt-2 text-body">{t('whoBody')}</p>
      </section>

      <section aria-labelledby="privacy-data">
        <h2 id="privacy-data" className="text-section font-semibold">
          {t('dataTitle')}
        </h2>
        <div className="card mt-3 overflow-x-auto">
          <table className="w-full text-body">
            <thead>
              <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                <th className="px-4 py-2 font-medium">{t('colWho')}</th>
                <th className="px-4 py-2 font-medium">{t('colData')}</th>
                <th className="px-4 py-2 font-medium">{t('colPurpose')}</th>
                <th className="px-4 py-2 font-medium">{t('colBasis')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.who} className="border-b border-steel-200 align-top last:border-0">
                  <td className="px-4 py-3 font-medium">{r.who}</td>
                  <td className="px-4 py-3">{r.data}</td>
                  <td className="px-4 py-3">{r.purpose}</td>
                  <td className="px-4 py-3">{r.basis}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="mt-3 text-metadata text-steel-500">{t('employeesNote')}</p>
      </section>

      <section aria-labelledby="privacy-sharing">
        <h2 id="privacy-sharing" className="text-section font-semibold">
          {t('sharingTitle')}
        </h2>
        <p className="mt-2 text-body">{t('sharingBody')}</p>
      </section>

      <section aria-labelledby="privacy-rights">
        <h2 id="privacy-rights" className="text-section font-semibold">
          {t('rightsTitle')}
        </h2>
        <ul className="mt-2 list-disc space-y-1 pl-5 text-body">
          {rights.map((r) => (
            <li key={r}>{r}</li>
          ))}
        </ul>
        <p className="mt-3 text-body">{t('rightsHow')}</p>
        <p className="mt-3 text-body">
          {t('unsubscribeText')}{' '}
          <a className="underline" href={`/${locale}/newsletter/unsubscribe`}>
            {t('unsubscribeLink')}
          </a>
        </p>
      </section>

      <section aria-labelledby="privacy-complaint">
        <h2 id="privacy-complaint" className="text-section font-semibold">
          {t('complaintTitle')}
        </h2>
        <p className="mt-2 text-body">{t('complaintBody')}</p>
      </section>

      <section aria-labelledby="privacy-contact" className="rounded-xl border border-steel-200 bg-surface p-5">
        <h2 id="privacy-contact" className="text-section font-semibold">
          {t('contactTitle')}
        </h2>
        <dl className="mt-3 grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-[auto,1fr]">
          {details
            .filter(([, value]) => value)
            .map(([label, value, kind]) => (
              <div key={label} className="contents">
                <dt className="text-steel-500">{label}</dt>
                <dd>
                  {kind === 'mail' ? (
                    <a className="underline" href={`mailto:${value}`}>
                      {value}
                    </a>
                  ) : kind === 'tel' ? (
                    <a className="underline" href={`tel:${value!.replace(/[^\d+]/g, '')}`}>
                      {value}
                    </a>
                  ) : (
                    value
                  )}
                </dd>
              </div>
            ))}
        </dl>
        {company.noticeUrl && (
          <p className="mt-4">
            <a className="btn-primary" href={company.noticeUrl} rel="noopener">
              {t('fullNotice')}
            </a>
          </p>
        )}
      </section>
    </article>
  );
}
