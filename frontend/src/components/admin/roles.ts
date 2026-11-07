// The roles and the rights an admin can hand out, in the order the screens list them.
// Their own module: the user list and the roles grid both need them, and each imports
// the other's component.

import type { Capability, Role } from '@/lib/api/types';

export const ROLES: Role[] = [
  'admin',
  'office',
  'sales',
  'accountant',
  'designer',
  'worker',
  'purchaser',
  'viewer',
];

/** What an admin can grant one user or tick for a role, mirroring Capability::grantable. */
export const GRANTABLE: Capability[] = [
  'view_prices',
  'view_invoices',
  'view_incoming_invoices',
  'view_sales',
  'view_contacts',
  'view_business_reports',
  'view_all_modules',
  'edit_partners',
  'edit_leads',
  'edit_orders',
  'change_stages',
  'handle_handovers',
  'manage_blockers',
  'upload_media',
  'delete_media',
  'view_original_images',
  'comment',
  'send_email',
  'issue_invoices',
  'manage_leave',
  'manage_recruitment',
  'annul_invoices',
  'manage_configuration',
  'manage_settings',
  'operate_system',
];
