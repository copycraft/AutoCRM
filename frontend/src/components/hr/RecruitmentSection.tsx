'use client';

import { useState } from 'react';
import { useFormatter, useTranslations } from 'next-intl';
import { useSearchParams } from 'next/navigation';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import * as Dialog from '@radix-ui/react-dialog';
import { ArrowLeft, Copy, ExternalLink, FileText, Plus } from 'lucide-react';
import { hrApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useRestoreFocus } from '@/hooks/useRestoreFocus';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { EmailValue, PhoneValue } from '@/components/ui/ContactLinks';
import { useToast } from '@/components/ui/Toasts';
import type { JobApplication, JobPosting } from '@/lib/api/types';

const STATUS_TONE: Record<string, StatusTone> = { draft: 'steel', published: 'done', closed: 'muted' };

/** HR's recruitment tab: job listings, each with its public form link, and who applied. */
export function RecruitmentSection() {
  // A notification links here with the listing already open.
  const initial = Number(useSearchParams()?.get('job')) || null;
  const [openId, setOpenId] = useState<number | null>(initial);
  return openId === null ? <JobList onOpen={setOpenId} /> : <JobDetail id={openId} onBack={() => setOpenId(null)} />;
}

function JobList({ onOpen }: { onOpen: (id: number) => void }) {
  const t = useTranslations('hr');
  const tc = useTranslations('common');
  const [creating, setCreating] = useState(false);
  const query = useQuery({ queryKey: qk.jobs, queryFn: () => hrApi.jobs() });
  const items = query.data?.items ?? [];

  return (
    <div className="mt-6 space-y-4">
      <div className="flex">
        <button className="btn-primary ml-auto" onClick={() => setCreating(true)}>
          <Plus className="h-4 w-4" aria-hidden />
          {t('newJob')}
        </button>
      </div>

      {query.isLoading ? (
        <LoadingState label={tc('loading')} />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : items.length === 0 ? (
        <EmptyState title={t('jobsEmpty')} hint={t('jobsEmptyHint')} />
      ) : (
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          {items.map((j) => (
            <li key={j.id} className="card">
              <div className="card-content space-y-1">
                <div className="flex items-start justify-between gap-2">
                  <p className="text-section font-semibold">{j.title}</p>
                  <StatusBadge tone={STATUS_TONE[j.status] ?? 'steel'}>{t(`jobStatus.${j.status}`)}</StatusBadge>
                </div>
                {j.location && <p className="text-body text-steel-500">{j.location}</p>}
                <p className="text-body">{t('applicantCount', { count: j.application_count })}</p>
              </div>
              <div className="card-footer justify-end">
                <button className="btn-ghost btn-sm" onClick={() => onOpen(j.id)}>
                  {t('applicants')}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {creating && (
        <JobDialog
          job={null}
          onClose={() => setCreating(false)}
          onSaved={(saved) => {
            setCreating(false);
            onOpen(saved.id);
          }}
        />
      )}
    </div>
  );
}

function JobDetail({ id, onBack }: { id: number; onBack: () => void }) {
  const t = useTranslations('hr');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const [editing, setEditing] = useState(false);
  const [confirm, setConfirm] = useState<'close' | 'delete' | null>(null);

  const jobs = useQuery({ queryKey: qk.jobs, queryFn: () => hrApi.jobs() });
  const apps = useQuery({ queryKey: qk.jobApplications(id), queryFn: () => hrApi.applications(id) });
  const job = jobs.data?.items.find((j) => j.id === id);

  const refresh = () => qc.invalidateQueries({ queryKey: qk.jobs });
  const onError = (e: unknown) => {
    setConfirm(null);
    toast.error(errorMessage(e, ter, ter('unknownError')));
  };
  const setStatus = useMutation({
    mutationFn: (action: 'publish' | 'close') => (action === 'publish' ? hrApi.publishJob(id) : hrApi.closeJob(id)),
    onSuccess: async () => {
      setConfirm(null);
      await refresh();
    },
    onError,
  });
  const remove = useMutation({
    mutationFn: () => hrApi.deleteJob(id),
    onSuccess: async () => {
      toast.success(t('jobDeleted'));
      await refresh();
      onBack();
    },
    onError,
  });

  if (jobs.isLoading) return <LoadingState label={tc('loading')} />;
  if (jobs.isError) return <ErrorState error={jobs.error} onRetry={() => void jobs.refetch()} />;
  if (!job) return <EmptyState title={t('jobsEmpty')} />;

  const applicants = apps.data?.items ?? [];
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(job.public_url);
      toast.success(t('linkCopied'));
    } catch {
      // No clipboard permission: the field is selectable, so the link is still one click away.
    }
  };

  return (
    <div className="mt-6 space-y-6">
      <button className="btn-ghost btn-sm -ml-2" onClick={onBack}>
        <ArrowLeft className="h-4 w-4" aria-hidden />
        {t('back')}
      </button>

      <div className="card">
        <div className="card-content space-y-4">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div>
              <h2 className="text-section font-semibold">{job.title}</h2>
              {job.location && <p className="text-body text-steel-500">{job.location}</p>}
            </div>
            <StatusBadge tone={STATUS_TONE[job.status] ?? 'steel'}>{t(`jobStatus.${job.status}`)}</StatusBadge>
          </div>
          {job.description && <p className="whitespace-pre-line text-body">{job.description}</p>}

          <div>
            <label className="label" htmlFor="job-link">
              {t('jobLink')}
            </label>
            <div className="flex gap-2">
              <input
                id="job-link"
                className="input flex-1"
                readOnly
                value={job.public_url}
                onFocus={(e) => e.currentTarget.select()}
              />
              <button type="button" className="btn-secondary" onClick={() => void copy()}>
                <Copy className="h-4 w-4" aria-hidden />
                {t('copyLink')}
              </button>
              <a className="btn-secondary" href={job.public_url} target="_blank" rel="noreferrer">
                <ExternalLink className="h-4 w-4" aria-hidden />
                {t('openForm')}
              </a>
            </div>
            {job.status === 'draft' && <p className="mt-1 text-metadata text-steel-500">{t('draftHint')}</p>}
          </div>
        </div>
        <div className="card-footer">
          {job.application_count === 0 && (
            <button className="btn-ghost mr-auto" disabled={remove.isPending} onClick={() => setConfirm('delete')}>
              {t('deleteJob')}
            </button>
          )}
          <button className="btn-ghost" onClick={() => setEditing(true)}>
            {tc('edit')}
          </button>
          {job.status === 'published' ? (
            <button className="btn-secondary" disabled={setStatus.isPending} onClick={() => setConfirm('close')}>
              {t('close')}
            </button>
          ) : (
            <button className="btn-primary" disabled={setStatus.isPending} onClick={() => setStatus.mutate('publish')}>
              {job.status === 'closed' ? t('reopen') : t('publish')}
            </button>
          )}
        </div>
      </div>

      <section className="space-y-3">
        <h3 className="text-section font-semibold">
          {t('applicants')} · {t('applicantCount', { count: job.application_count })}
        </h3>
        {apps.isLoading ? (
          <LoadingState label={tc('loading')} />
        ) : apps.isError ? (
          <ErrorState error={apps.error} onRetry={() => void apps.refetch()} />
        ) : applicants.length === 0 ? (
          <EmptyState title={t('applicantsEmpty')} hint={t('applicantsEmptyHint')} />
        ) : (
          <ul className="space-y-4">
            {applicants.map((a) => (
              <li key={a.id}>
                <Applicant application={a} jobId={id} />
              </li>
            ))}
          </ul>
        )}
      </section>

      {editing && (
        <JobDialog
          key={job.id}
          job={job}
          onClose={() => setEditing(false)}
          onSaved={() => setEditing(false)}
        />
      )}
      <ConfirmDialog
        open={confirm === 'close'}
        title={t('closeConfirmTitle')}
        body={t('closeConfirmBody')}
        onConfirm={() => setStatus.mutate('close')}
        onClose={() => setConfirm(null)}
        busy={setStatus.isPending}
      />
      <ConfirmDialog
        open={confirm === 'delete'}
        title={t('deleteJobConfirmTitle')}
        body={t('deleteJobConfirmBody')}
        onConfirm={() => remove.mutate()}
        onClose={() => setConfirm(null)}
        busy={remove.isPending}
      />
    </div>
  );
}

function Applicant({ application: a, jobId }: { application: JobApplication; jobId: number }) {
  const t = useTranslations('hr');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const format = useFormatter();
  const qc = useQueryClient();
  const toast = useToast();
  const [notes, setNotes] = useState(a.notes ?? '');
  const [confirmDelete, setConfirmDelete] = useState(false);

  const saveNotes = useMutation({
    mutationFn: () => hrApi.saveApplicationNotes(a.id, notes.trim() || null),
    onSuccess: async () => {
      toast.success(t('notesSaved'));
      await qc.invalidateQueries({ queryKey: qk.jobApplications(jobId) });
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });
  const remove = useMutation({
    mutationFn: () => hrApi.deleteApplication(a.id),
    onSuccess: async () => {
      setConfirmDelete(false);
      toast.success(t('applicantDeleted'));
      await qc.invalidateQueries({ queryKey: qk.jobs });
    },
    onError: (e) => {
      setConfirmDelete(false);
      toast.error(errorMessage(e, ter, ter('unknownError')));
    },
  });
  const dirty = notes.trim() !== (a.notes ?? '').trim();

  return (
    <div className="card">
      <div className="card-content space-y-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <p className="text-section font-semibold">{a.full_name}</p>
          <p className="text-metadata text-steel-500">
            {t('appliedAt')} {format.dateTime(new Date(a.created_at), { dateStyle: 'medium', timeStyle: 'short' })}
          </p>
        </div>
        <dl className="grid grid-cols-[auto,1fr] gap-x-4 gap-y-0.5 text-body">
          <dt className="text-steel-500">{t('email')}</dt>
          <dd>
            <EmailValue value={a.email} />
          </dd>
          <dt className="text-steel-500">{t('phone')}</dt>
          <dd>
            <PhoneValue value={a.phone} />
          </dd>
          <dt className="text-steel-500">{t('age')}</dt>
          <dd>{t('ageYears', { age: a.age })}</dd>
          {a.city && (
            <>
              <dt className="text-steel-500">{t('city')}</dt>
              <dd>{a.city}</dd>
            </>
          )}
          <dt className="text-steel-500">{t('resume')}</dt>
          <dd>
            {a.resume_url ? (
              <a className="inline-flex items-center gap-1 text-cold underline" href={a.resume_url}>
                <FileText className="h-4 w-4" aria-hidden />
                {t('downloadResume')} ({a.resume_filename})
              </a>
            ) : (
              <span className="text-steel-500">{t('resumeUnavailable')}</span>
            )}
          </dd>
        </dl>
        {a.message && (
          <div>
            <p className="text-metadata text-steel-500">{t('applicantMessage')}</p>
            <p className="whitespace-pre-line text-body">{a.message}</p>
          </div>
        )}
        <div>
          <label className="label" htmlFor={`notes-${a.id}`}>
            {t('notes')}
          </label>
          <textarea
            id={`notes-${a.id}`}
            className="input min-h-20"
            value={notes}
            maxLength={10000}
            onChange={(e) => setNotes(e.target.value)}
          />
          <p className="mt-1 text-metadata text-steel-500">{t('notesHint')}</p>
        </div>
      </div>
      <div className="card-footer">
        <button className="btn-ghost mr-auto" onClick={() => setConfirmDelete(true)} disabled={remove.isPending}>
          {t('deleteApplicant')}
        </button>
        <button className="btn-secondary" disabled={!dirty || saveNotes.isPending} onClick={() => saveNotes.mutate()}>
          {saveNotes.isPending ? tc('saving') : tc('save')}
        </button>
      </div>
      <ConfirmDialog
        open={confirmDelete}
        title={t('deleteApplicantConfirmTitle')}
        body={t('deleteApplicantConfirmBody')}
        onConfirm={() => remove.mutate()}
        onClose={() => setConfirmDelete(false)}
        busy={remove.isPending}
      />
    </div>
  );
}

function JobDialog({
  job,
  onClose,
  onSaved,
}: {
  job: JobPosting | null;
  onClose: () => void;
  onSaved: (saved: JobPosting) => void;
}) {
  const t = useTranslations('hr');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  useRestoreFocus(true);
  const [title, setTitle] = useState(job?.title ?? '');
  const [location, setLocation] = useState(job?.location ?? '');
  const [description, setDescription] = useState(job?.description ?? '');
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: () => {
      // Blank clears on PATCH; on create the backend treats it as absent.
      const body = { title, location: location.trim() || null, description: description.trim() || null };
      return job ? hrApi.updateJob(job.id, body) : hrApi.createJob(body);
    },
    onSuccess: async (saved) => {
      toast.success(t('jobSaved'));
      await qc.invalidateQueries({ queryKey: qk.jobs });
      onSaved(saved);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  const canSave = title.trim() !== '' && !save.isPending;

  return (
    <Dialog.Root open onOpenChange={(open) => !open && !save.isPending && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[92vw] max-w-lg -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (canSave) save.mutate();
            }}
          >
            <div className="card-header">
              <Dialog.Title className="text-section font-semibold">{job ? t('editJob') : t('newJob')}</Dialog.Title>
              <Dialog.Description className="sr-only">{t('jobDescriptionHint')}</Dialog.Description>
            </div>
            <div className="card-content space-y-4">
              {error && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                  {error}
                </p>
              )}
              <div>
                <label className="label" htmlFor="job-title">
                  {t('jobTitle')} *
                </label>
                <input
                  id="job-title"
                  className="input"
                  value={title}
                  maxLength={200}
                  onChange={(e) => setTitle(e.target.value)}
                  autoFocus
                />
              </div>
              <div>
                <label className="label" htmlFor="job-location">
                  {t('jobLocation')}
                </label>
                <input
                  id="job-location"
                  className="input"
                  value={location}
                  maxLength={200}
                  onChange={(e) => setLocation(e.target.value)}
                />
              </div>
              <div>
                <label className="label" htmlFor="job-description">
                  {t('jobDescription')}
                </label>
                <textarea
                  id="job-description"
                  className="input min-h-32"
                  value={description}
                  maxLength={10000}
                  onChange={(e) => setDescription(e.target.value)}
                />
                <p className="mt-1 text-metadata text-steel-500">{t('jobDescriptionHint')}</p>
              </div>
            </div>
            <div className="card-footer">
              <button type="button" className="btn-ghost" onClick={onClose} disabled={save.isPending}>
                {tc('cancel')}
              </button>
              <button type="submit" className="btn-primary" disabled={!canSave}>
                {save.isPending ? tc('saving') : tc('save')}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
