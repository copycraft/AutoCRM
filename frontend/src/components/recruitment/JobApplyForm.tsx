'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery } from '@tanstack/react-query';
import { CheckCircle2, Snowflake } from 'lucide-react';
import { recruitmentApi } from '@/lib/api/endpoints';
import { ApiError } from '@/lib/api/errors';
import { LoadingState } from '@/components/ui/LoadingState';

const MAX_RESUME_BYTES = 10 * 1024 * 1024;
const RESUME_EXTENSIONS = ['pdf', 'doc', 'docx', 'odt', 'rtf'];
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

type Field = 'fullName' | 'email' | 'phone' | 'age' | 'resume';
type Errors = Partial<Record<Field, string>>;

/** The public application form of one job listing: details and a resume, no login. */
export function JobApplyForm({ slug }: { slug: string }) {
  const t = useTranslations('apply');
  const job = useQuery({
    queryKey: ['public-job', slug],
    queryFn: () => recruitmentApi.job(slug),
    retry: false,
  });

  let body: React.ReactNode;
  if (job.isLoading) {
    body = <LoadingState label={t('loading')} />;
  } else if (job.isError) {
    const notFound = job.error instanceof ApiError && job.error.status === 404;
    body = <Notice title={t('notFound')} body={notFound ? undefined : t('sendFailed')} />;
  } else if (job.data!.status === 'closed') {
    body = <Notice title={t('closedTitle')} body={t('closedBody')} />;
  } else {
    body = <Form slug={slug} />;
  }

  return (
    <div className="min-h-screen bg-panel px-4 py-8">
      <div className="mx-auto w-full max-w-xl space-y-4">
        <div className="flex items-center gap-3">
          <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-steel-900 text-surface">
            <Snowflake className="h-5 w-5" aria-hidden />
          </span>
          <p className="text-section font-semibold">{t('brand')}</p>
        </div>
        {job.data && (
          <div className="card">
            <div className="card-content space-y-1">
              <h1 className="text-section font-semibold">{job.data.title}</h1>
              {job.data.location && <p className="text-body text-steel-500">{job.data.location}</p>}
              {job.data.description && <p className="whitespace-pre-line pt-2 text-body">{job.data.description}</p>}
            </div>
          </div>
        )}
        {body}
      </div>
    </div>
  );
}

function Notice({ title, body }: { title: string; body?: string }) {
  return (
    <div className="card">
      <div className="card-content space-y-1">
        <p className="text-section font-semibold">{title}</p>
        {body && <p className="text-body text-steel-500">{body}</p>}
      </div>
    </div>
  );
}

function Form({ slug }: { slug: string }) {
  const t = useTranslations('apply');
  const [values, setValues] = useState({ fullName: '', email: '', phone: '', age: '', city: '', message: '', company: '' });
  const [resume, setResume] = useState<File | null>(null);
  const [errors, setErrors] = useState<Errors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const set = (k: keyof typeof values) => (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) =>
    setValues((v) => ({ ...v, [k]: e.target.value }));

  const submit = useMutation({
    mutationFn: () => {
      const form = new FormData();
      form.set('full_name', values.fullName.trim());
      form.set('email', values.email.trim());
      form.set('phone', values.phone.trim());
      form.set('age', values.age.trim());
      form.set('city', values.city.trim());
      form.set('message', values.message.trim());
      form.set('company', values.company);
      form.set('resume', resume!, resume!.name);
      return recruitmentApi.apply(slug, form);
    },
    onSuccess: () => setDone(true),
    onError: (e) => {
      // The applicant sees Hungarian, never the backend's English detail.
      const msg = e instanceof ApiError ? e.backendMessage : '';
      setServerError(msg.includes('already applied') ? t('alreadyApplied') : t('sendFailed'));
    },
  });

  const validate = (): Errors => {
    const out: Errors = {};
    if (!values.fullName.trim()) out.fullName = t('required');
    if (!values.email.trim()) out.email = t('required');
    else if (!EMAIL.test(values.email.trim())) out.email = t('invalidEmail');
    if (!values.phone.trim()) out.phone = t('required');
    const age = Number(values.age.trim());
    if (!values.age.trim()) out.age = t('required');
    else if (!Number.isInteger(age) || age < 14 || age > 100) out.age = t('invalidAge');
    if (!resume) out.resume = errors.resume ?? t('required');
    return out;
  };

  const pickResume = (file: File | undefined) => {
    setServerError(null);
    if (!file) {
      setResume(null);
      return;
    }
    const ext = file.name.split('.').pop()?.toLowerCase() ?? '';
    if (!RESUME_EXTENSIONS.includes(ext)) {
      setResume(null);
      setErrors((x) => ({ ...x, resume: t('resumeBadType') }));
      return;
    }
    if (file.size > MAX_RESUME_BYTES) {
      setResume(null);
      setErrors((x) => ({ ...x, resume: t('resumeTooBig') }));
      return;
    }
    setErrors((x) => ({ ...x, resume: undefined }));
    setResume(file);
  };

  if (done) {
    return (
      <div className="card">
        <div className="card-content flex gap-3">
          <CheckCircle2 className="h-6 w-6 shrink-0 text-done" aria-hidden />
          <div className="space-y-1">
            <p className="text-section font-semibold">{t('doneTitle')}</p>
            <p className="text-body text-steel-500">{t('doneBody')}</p>
          </div>
        </div>
      </div>
    );
  }

  const fieldError = (k: Field) =>
    errors[k] && (
      <p id={`apply-${k}-error`} className="mt-1 text-metadata text-steel-900">
        {errors[k]}
      </p>
    );

  return (
    <form
      className="card"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        setServerError(null);
        const found = validate();
        setErrors(found);
        if (Object.keys(found).length === 0) submit.mutate();
      }}
    >
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('formTitle')}</h2>
      </div>
      <div className="card-content space-y-4">
        <div>
          <label className="label" htmlFor="apply-name">
            {t('fullName')} *
          </label>
          <input
            id="apply-name"
            className="input"
            autoComplete="name"
            maxLength={200}
            value={values.fullName}
            onChange={set('fullName')}
            aria-invalid={!!errors.fullName}
            aria-describedby={errors.fullName ? 'apply-fullName-error' : undefined}
          />
          {fieldError('fullName')}
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label className="label" htmlFor="apply-email">
              {t('email')} *
            </label>
            <input
              id="apply-email"
              type="email"
              className="input"
              autoComplete="email"
              maxLength={300}
              value={values.email}
              onChange={set('email')}
              aria-invalid={!!errors.email}
              aria-describedby={errors.email ? 'apply-email-error' : undefined}
            />
            {fieldError('email')}
          </div>
          <div>
            <label className="label" htmlFor="apply-phone">
              {t('phone')} *
            </label>
            <input
              id="apply-phone"
              type="tel"
              className="input"
              autoComplete="tel"
              maxLength={50}
              value={values.phone}
              onChange={set('phone')}
              aria-invalid={!!errors.phone}
              aria-describedby={errors.phone ? 'apply-phone-error' : undefined}
            />
            {fieldError('phone')}
          </div>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label className="label" htmlFor="apply-age">
              {t('age')} *
            </label>
            <input
              id="apply-age"
              inputMode="numeric"
              className="input"
              maxLength={3}
              value={values.age}
              onChange={set('age')}
              aria-invalid={!!errors.age}
              aria-describedby={errors.age ? 'apply-age-error' : undefined}
            />
            {fieldError('age')}
          </div>
          <div>
            <label className="label" htmlFor="apply-city">
              {t('city')}
            </label>
            <input
              id="apply-city"
              className="input"
              autoComplete="address-level2"
              maxLength={200}
              value={values.city}
              onChange={set('city')}
            />
          </div>
        </div>
        <div>
          <label className="label" htmlFor="apply-message">
            {t('message')}
          </label>
          <textarea
            id="apply-message"
            className="input min-h-24"
            maxLength={5000}
            value={values.message}
            onChange={set('message')}
          />
          <p className="mt-1 text-metadata text-steel-500">{t('messageHint')}</p>
        </div>
        <div>
          <label className="label" htmlFor="apply-resume">
            {t('resume')} *
          </label>
          <input
            id="apply-resume"
            type="file"
            className="input"
            accept=".pdf,.doc,.docx,.odt,.rtf"
            onChange={(e) => pickResume(e.target.files?.[0])}
            aria-invalid={!!errors.resume}
            aria-describedby={errors.resume ? 'apply-resume-error' : undefined}
          />
          <p className="mt-1 text-metadata text-steel-500">{t('resumeHint')}</p>
          {fieldError('resume')}
        </div>
        {/* Honeypot: invisible to people, tempting to bots. A filled one is dropped server-side. */}
        <div aria-hidden className="absolute -left-[9999px] h-0 w-0 overflow-hidden">
          <label htmlFor="apply-company">Company</label>
          <input
            id="apply-company"
            tabIndex={-1}
            autoComplete="off"
            value={values.company}
            onChange={set('company')}
          />
        </div>
        {serverError && (
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
            {serverError}
          </p>
        )}
        <p className="text-metadata text-steel-500">{t('privacy')}</p>
      </div>
      <div className="card-footer">
        <button type="submit" className="btn-primary w-full sm:w-auto" disabled={submit.isPending}>
          {submit.isPending ? t('submitting') : t('submit')}
        </button>
      </div>
    </form>
  );
}
