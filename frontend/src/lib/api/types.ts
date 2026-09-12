// Friendly names for the generated API types. Aliases only: every shape lives in
// schema.gen.ts, generated from openapi/openapi.json. Never add hand-written fields here.
import type { components, operations } from './schema.gen';

type Schemas = components['schemas'];

/** Query parameters of an operation, by operation id. */
export type QueryOf<Op extends keyof operations> = NonNullable<operations[Op]['parameters']['query']>;

export type Currency = Schemas['Currency'];
export type Role = Schemas['Role'];
export type PartnerKind = Schemas['PartnerKind'];
export type ImageCategory = Schemas['ImageCategory'];
export type StageEntity = Schemas['StageEntity'];

export type SessionUser = Schemas['SessionUser'];
export type LoginBody = Schemas['LoginBody'];
export type LoginResponse = Schemas['LoginResponse'];
export type MeResponse = Schemas['MeResponse'];
export type ChangePasswordBody = Schemas['ChangePasswordBody'];
export type User = Schemas['User'];

export type Partner = Schemas['Partner'];
export type PartnerDetail = Schemas['PartnerDetail'];
export type CreatePartner = Schemas['CreatePartner'];
export type PatchPartner = Schemas['PatchPartner'];
export type Contact = Schemas['Contact'];
export type ContactBody = Schemas['ContactBody'];

export type Lead = Schemas['Lead'];
export type LeadSummary = Schemas['LeadSummary'];
export type LeadDetail = Schemas['LeadDetail'];
export type LeadBody = Schemas['LeadBody'];

export type Order = Schemas['Order'];
export type OrderSummary = Schemas['OrderSummary'];
export type OrderDetail = Schemas['OrderDetail'];
export type OrderBody = Schemas['OrderBody'];
export type PatchOrder = Schemas['PatchOrder'];
export type ItemView = Schemas['ItemView'];
export type AddItem = Schemas['AddItem'];
export type PatchItem = Schemas['PatchItem'];

export type StageBody = Schemas['StageBody'];
export type StageChange = Schemas['StageChange'];
export type StageDefinition = Schemas['StageDefinition'];
export type StageEntry = Schemas['StageEntry'];
export type TransitionOption = Schemas['TransitionOption'];

/** List envelope shared by every collection endpoint. */
export interface Items<T> {
  items: T[];
}
export type AuditEntry = Schemas['AuditEntry'];
export type OrderNote = Schemas['OrderNote'];
export type RawImportView = Schemas['RawImportView'];
export type Blocker = Schemas['Blocker'];
export type ProjectType = Schemas['ProjectType'];
export type Settings = Schemas['Settings'];
export type EmailTransportBody = Schemas['EmailTransportBody'];
export type UserSettings = Schemas['UserSettings'];
export type EmailSummary = Schemas['EmailSummary'];
export type EmailMessage = Schemas['EmailMessage'];
export type EmailStatus = Schemas['EmailStatus'];
