// Uploads from the browser, the same three steps the phone takes: ask for a ticket (the
// server signs exactly this file: size and sha256), PUT the bytes straight to object
// storage, then hand the ticket back. Nothing passes through the API server, and a file
// already on the record is recognised by its hash instead of stored twice.
//
// The object store must allow the web app's origin to PUT (CORS). MinIO does by default;
// on AWS S3 or Backblaze add a CORS rule for PUT from the app's address.

import { mediaApi } from '@/lib/api/endpoints';
import type { components } from '@/lib/api/schema.gen';

type S = components['schemas'];
export type UploadTarget = S['UploadTarget'];

export type UploadOutcome =
  | { status: 'created' | 'existing'; completed: S['Completed'] }
  | { status: 'already_uploaded' };

/** Browsers refuse to set these themselves; the PUT carries them anyway. */
const FORBIDDEN = new Set(['host', 'content-length']);

export async function sha256Hex(file: Blob): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', await file.arrayBuffer());
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

/** The type the server sees: what the browser says, or a generic one when it says nothing. */
export function contentTypeOf(file: File): string {
  if (file.type) return file.type;
  const ext = file.name.split('.').pop()?.toLowerCase();
  if (ext === 'heic' || ext === 'heif') return 'image/heic';
  if (ext === 'dxf') return 'image/vnd.dxf';
  if (ext === 'dwg') return 'image/vnd.dwg';
  return 'application/octet-stream';
}

export async function uploadFile(
  owner: { order: number } | { lead: number },
  file: File,
  target: UploadTarget,
): Promise<UploadOutcome> {
  const body: S['UploadRequest'] = {
    target,
    filename: file.name,
    content_type: contentTypeOf(file),
    byte_size: file.size,
    sha256: await sha256Hex(file),
  };
  const ticket =
    'order' in owner
      ? await mediaApi.requestUpload(owner.order, body)
      : await mediaApi.requestLeadUpload(owner.lead, body);
  if (ticket.status === 'already_uploaded') return { status: 'already_uploaded' };

  const headers = new Headers();
  for (const [name, value] of ticket.upload.headers) {
    if (name && value !== undefined && !FORBIDDEN.has(name.toLowerCase())) headers.set(name, value);
  }
  const put = await fetch(ticket.upload.url, { method: ticket.upload.method, headers, body: file });
  if (!put.ok) {
    throw new Error(`storage refused the upload (${put.status})`);
  }
  const completed = await mediaApi.completeUpload({ ticket: ticket.ticket });
  const created = 'created' in completed ? completed.created : true;
  return { status: created ? 'created' : 'existing', completed };
}
