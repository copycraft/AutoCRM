// Friendly names for the generated API types. Aliases only: every shape lives in
// schema.gen.ts, generated from openapi/openapi.json. Never add hand-written fields here.
import type { components, operations } from './schema.gen';

type Schemas = components['schemas'];

/** Query parameters of an operation, by operation id. */
export type QueryOf<Op extends keyof operations> = NonNullable<operations[Op]['parameters']['query']>;

export type Currency = Schemas['Currency'];
export type Role = Schemas['Role'];
export type Capability = Schemas['Capability'];
export type PartnerKind = Schemas['PartnerKind'];
export type ImageCategory = Schemas['ImageCategory'];
export type StageEntity = Schemas['StageEntity'];

export type SessionUser = Schemas['SessionUser'];
export type LoginBody = Schemas['LoginBody'];
export type LoginResponse = Schemas['LoginResponse'];
export type MeResponse = Schemas['MeResponse'];
export type ChangePasswordBody = Schemas['ChangePasswordBody'];
export type User = Schemas['User'];
export type Employee = Schemas['Employee'];
export type JobPosting = Schemas['JobPosting'];
export type JobBody = Schemas['JobBody'];
export type JobApplication = Schemas['Application'];
export type PublicJob = Schemas['PublicJob'];
export type EmployeeBody = Schemas['EmployeeBody'];
export type Absence = Schemas['Absence'];
export type AbsenceBody = Schemas['AbsenceBody'];
export type LeaveKind = Schemas['LeaveKind'];
export type LeaveBalance = Schemas['LeaveBalance'];
export type Notification = Schemas['Notification'];
export type NotificationList = Schemas['NotificationList'];

export type Partner = Schemas['Partner'];
export type PartnerDetail = Schemas['PartnerDetail'];
export type CreatePartner = Schemas['CreatePartner'];
export type PatchPartner = Schemas['PatchPartner'];
export type Contact = Schemas['Contact'];
export type ContactBody = Schemas['ContactBody'];

export type Invoice = Schemas['Invoice'];
export type InvoiceDetail = Schemas['InvoiceDetail'];
export type InvoiceStatus = Schemas['InvoiceStatus'];
export type InvoiceKind = Schemas['InvoiceKind'];
export type InvoiceLine = Schemas['InvoiceLine'];
export type NavMessage = Schemas['NavMessage'];
export type BilledInvoice = Schemas['BilledInvoice'];
export type BilledProforma = Schemas['BilledProforma'];
export type ChainStep = Schemas['ChainStep'];
export type Proforma = Schemas['Proforma'];
export type IssueRequest = Schemas['IssueRequest'];
export type StornoRequest = Schemas['StornoRequest'];
export type AnnulRequest = Schemas['AnnulRequest'];
export type ProformaRequest = Schemas['ProformaRequest'];

export type Lead = Schemas['Lead'];
export type LeadSummary = Schemas['LeadSummary'];
export type LeadDetail = Schemas['LeadDetail'];
export type LeadRow = Schemas['LeadRow'];
export type LeadTag = Schemas['LeadTag'];
export type LeadTagRef = Schemas['LeadTagRef'];
export type NewsletterTag = Schemas['NewsletterTag'];
export type SubscriberRow = Schemas['SubscriberRow'];
export type IncomingInvoice = Schemas['IncomingInvoice'];
export type EmployeeStatus = Schemas['EmployeeStatus'];
export type TimelineEvent = Schemas['TimelineEvent'];
export type AssistantReply = Schemas['AssistantReply'];
export type FilterSuggestion = Schemas['FilterSuggestion'];
export type EmailTemplate = Schemas['EmailTemplate'];
export type FollowupStep = Schemas['FollowupStep'];
export type Followup = Schemas['Followup'];
export type LeadBody = Schemas['LeadBody'];

export type Order = Schemas['Order'];
export type OrderSummary = Schemas['OrderSummary'];
export type OrderDetail = Schemas['OrderDetail'];
export type OrderValue = Schemas['OrderValue'];
export type PartnerRef = Schemas['PartnerRef'];
export type StageView = Schemas['StageView'];
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
export type Vehicle = Schemas['Vehicle'];
export type OrderSpec = Schemas['OrderSpec'];
export type SpecBody = Schemas['SpecBody'];
export type Document = Schemas['Document'];
export type RawImportView = Schemas['RawImportView'];
export type Blocker = Schemas['Blocker'];
export type ProjectType = Schemas['ProjectType'];
export type Settings = Schemas['Settings'];
export type EmailTransportBody = Schemas['EmailTransportBody'];
export type UserSettings = Schemas['UserSettings'];
export type EmailSummary = Schemas['EmailSummary'];
export type EmailMessage = Schemas['EmailMessage'];
export type EmailStatus = Schemas['EmailStatus'];

export type Inspection = Schemas['Inspection'];
export type InspectionDetail = Schemas['InspectionDetail'];
export type InspectionPhoto = Schemas['InspectionPhoto'];
export type InspectionDamage = Schemas['InspectionDamage'];
export type InspectionVerdict = Schemas['InspectionVerdict'];
export type InspectionSignature = Schemas['InspectionSignature'];
export type InspectionNote = Schemas['InspectionNote'];
export type InspectionComparison = Schemas['Comparison'];
export type ZoneTemplate = Schemas['ZoneTemplate'];
export type Lookups = Schemas['Lookups'];
export type LookupItem = Schemas['LookupItem'];

// 0048: lead sources, comments, the yard, incidents, photos and documents.
export type LeadSource = Schemas['LeadSource'];
export type Comment = Schemas['Comment'];
export type Mentionable = Schemas['Mentionable'];
export type YardLocation = Schemas['YardLocation'];
export type YardVehicle = Schemas['YardVehicle'];
export type YardBoard = Schemas['Board'];
export type VehicleMove = Schemas['VehicleMove'];
export type CurrentLocation = Schemas['CurrentLocation'];
export type Incident = Schemas['Incident'];
export type IncidentBody = Schemas['IncidentBody'];
export type ImageView = Schemas['ImageView'];
export type DocumentView = Schemas['DocumentView'];
export type Annotations = Schemas['Annotations'];
export type InspectionTyre = Schemas['InspectionTyre'];
export type InspectionVideoView = Schemas['InspectionVideoView'];
export type StagePhotoCategory = Schemas['StagePhotoCategory'];
