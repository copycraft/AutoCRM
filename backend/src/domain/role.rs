//! Roles and capabilities, checked at the handler boundary.
//! No policy DSL: adding a capability forces a decision for every role (exhaustive match).
//!
//! Capabilities come in two kinds. Writing ones (`Edit…`, `IssueInvoices`, …) say what a
//! person may change. Seeing ones (`View…`) say what they may read: a workshop hand needs the
//! job, the vehicle and their own department's part, not the price, the invoices or the
//! customer's phone number. The server leaves out what a person may not see — hiding it on a
//! screen would still send it to the phone.
//!
//! [`Role::can`] is each role's default. The live set per role is the admin's grid
//! (`role_permissions`, 0061), seeded from these defaults; admins always have everything.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    /// Iroda: the office runs the whole job, money included.
    Office,
    /// Értékesítő (0060): leads, quotes and customers; prices, but not the invoicing.
    Sales,
    /// Könyvelő (0060): invoices, payments and supplier invoices; no production edits.
    Accountant,
    /// Tervező: plans the build and moves it along. Stages, blockers, photos, handovers and
    /// their own department's part — no prices, no invoices, no customer contacts.
    Designer,
    /// Munkás (0062): builds it. Photos, comments and their own department's part.
    Worker,
    /// Anyagbeszerző (0062): buys the materials, and takes the vehicles in and hands them
    /// back — so the handovers, the customer's phone number and the supplier invoices.
    Purchaser,
    /// Csak olvasó: sees what the office sees, changes nothing.
    Viewer,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Admin,
        Role::Office,
        Role::Sales,
        Role::Accountant,
        Role::Designer,
        Role::Worker,
        Role::Purchaser,
        Role::Viewer,
    ];

    /// The stored and serialized name, e.g. `office`.
    pub fn key(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Office => "office",
            Role::Sales => "sales",
            Role::Accountant => "accountant",
            Role::Designer => "designer",
            Role::Worker => "worker",
            Role::Purchaser => "purchaser",
            Role::Viewer => "viewer",
        }
    }

    pub fn from_key(key: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.key() == key)
    }
}

/// Every authenticated user may read orders (without their prices), partners (without
/// their contact details), images and production reports. Capabilities cover the rest.
/// The role gives a default set; an admin can change a role's set in the grid, or grant
/// any [`Capability::grantable`] one to a single user on top of it (`users.permissions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ManageUsers,
    ManageSettings,
    /// Stage definitions, project types, email templates, suppression list.
    ManageConfiguration,
    /// Retry dead jobs, retry/cancel any email.
    OperateSystem,
    EditPartners,
    EditLeads,
    EditOrders,
    ChangeStages,
    ManageBlockers,
    UploadMedia,
    DeleteMedia,
    ViewOriginalImages,
    SendEmail,
    /// Issue an invoice, storno one, or render a proforma. Reporting to the tax authority
    /// under the company's own technical user is office work, not production work.
    IssueInvoices,
    /// Technically annul a data report. Admin only: it says the report should never have
    /// existed, and a person then has to approve it in NAV's own portal.
    AnnulInvoices,
    /// The HR module: the staff directory with personal phone numbers, the documents and
    /// the checklists. Admins always have it; anyone else needs the per-user `hr_access`
    /// flag an admin sets, which is checked in [`holds`], because a role alone cannot say
    /// it. The flag also brings [`ManageLeave`](Capability::ManageLeave) and
    /// [`ManageRecruitment`](Capability::ManageRecruitment): the whole module.
    AccessHr,
    /// Write comments on orders and leads. The shop floor talks about jobs too, so
    /// designers may; viewers read only.
    Comment,
    /// Prices: line prices and totals, an order's value, quoted amounts (0061).
    ViewPrices,
    /// Invoices, proformas, payments and customer statements (0061).
    ViewInvoices,
    /// Supplier invoices (0061).
    ViewIncomingInvoices,
    /// Leads, quotations, follow-ups and the correspondence with customers (0061).
    ViewSales,
    /// A customer's email, phone, address and contacts; the name is always shown (0061).
    ViewContacts,
    /// Revenue and sales reports; production reports are open to everyone (0061).
    ViewBusinessReports,
    /// Every department's technical content; without it, only one's own departments' (0061).
    ViewAllModules,
    /// Átvétel and átadás: the walkaround protocols when a vehicle comes in and goes back,
    /// signatures included (0063). Was `ChangeStages`; taking a car in is not planning it.
    HandleHandovers,
    /// Leave and absences: record them, attach the doctor's note, read the yearly summary.
    /// The staff list comes with it by name only — no phone numbers, no documents — so a
    /// workshop head can keep their people's leave without the rest of HR.
    ManageLeave,
    /// Recruitment: job listings, their ads and what they cost, and the applicants with
    /// their resumes. An office hand can run the hiring without the staff directory.
    ManageRecruitment,
}

impl Capability {
    pub const ALL: [Capability; 27] = {
        use Capability::*;
        [
            ManageUsers,
            ManageSettings,
            ManageConfiguration,
            OperateSystem,
            EditPartners,
            EditLeads,
            EditOrders,
            ChangeStages,
            ManageBlockers,
            UploadMedia,
            DeleteMedia,
            ViewOriginalImages,
            SendEmail,
            IssueInvoices,
            AnnulInvoices,
            AccessHr,
            Comment,
            ViewPrices,
            ViewInvoices,
            ViewIncomingInvoices,
            ViewSales,
            ViewContacts,
            ViewBusinessReports,
            ViewAllModules,
            HandleHandovers,
            ManageLeave,
            ManageRecruitment,
        ]
    };

    /// What an admin may grant one user beyond their role, or tick for a role in the grid.
    /// Not `ManageUsers`: whoever has it can grant themselves everything else, so it is the
    /// admin role itself. Not `AccessHr` either: that has its own `hr_access` flag.
    pub fn grantable(self) -> bool {
        !matches!(self, Capability::ManageUsers | Capability::AccessHr)
    }

    /// What the per-user `hr_access` flag gives: the whole HR module, its parts included.
    pub fn comes_with_hr_access(self) -> bool {
        matches!(
            self,
            Capability::AccessHr | Capability::ManageLeave | Capability::ManageRecruitment
        )
    }

    /// The stored and serialized name, e.g. `edit_orders`.
    pub fn key(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    pub fn from_key(key: &str) -> Option<Capability> {
        serde_json::from_value(serde_json::Value::String(key.to_owned())).ok()
    }
}

impl Role {
    /// The role's default set: what the grid is seeded with, and what applies where no grid
    /// was loaded (tests, internal callers).
    pub fn can(self, capability: Capability) -> bool {
        use Capability::*;
        use Role::*;
        match capability {
            ManageUsers | ManageSettings | ManageConfiguration | OperateSystem | AnnulInvoices
            | AccessHr | ManageLeave | ManageRecruitment => matches!(self, Admin),
            EditOrders | DeleteMedia | ViewOriginalImages => matches!(self, Admin | Office),
            EditLeads => matches!(self, Admin | Office | Sales),
            EditPartners | SendEmail => matches!(self, Admin | Office | Sales | Accountant),
            IssueInvoices => matches!(self, Admin | Office | Accountant),
            ChangeStages | ManageBlockers => matches!(self, Admin | Office | Designer),
            HandleHandovers => matches!(self, Admin | Office | Designer | Purchaser),
            UploadMedia => matches!(self, Admin | Office | Sales | Designer | Worker | Purchaser),
            Comment => {
                matches!(
                    self,
                    Admin | Office | Sales | Accountant | Designer | Worker | Purchaser
                )
            }
            ViewContacts => matches!(
                self,
                Admin | Office | Sales | Accountant | Purchaser | Viewer
            ),
            ViewPrices | ViewBusinessReports | ViewAllModules => {
                matches!(self, Admin | Office | Sales | Accountant | Viewer)
            }
            ViewInvoices => matches!(self, Admin | Office | Accountant | Viewer),
            ViewIncomingInvoices => {
                matches!(self, Admin | Office | Accountant | Purchaser | Viewer)
            }
            ViewSales => matches!(self, Admin | Office | Sales | Viewer),
        }
    }
}

/// Whether a person holds a capability — the one rule the session check and the `me`
/// response both use, so the screens show exactly what the API will accept.
///
/// - Admins hold everything (they are not in the grid: nobody can untick the last admin).
/// - Otherwise the role's set: the admin's grid when loaded (`grid`), else the defaults.
/// - `hr_access` brings the HR module ([`Capability::comes_with_hr_access`]).
/// - Per-user grants add any [`Capability::grantable`] one.
pub fn holds(
    role: Role,
    grid: Option<&[Capability]>,
    hr_access: bool,
    grants: &[Capability],
    capability: Capability,
) -> bool {
    let from_role = match (grid, role) {
        (_, Role::Admin) => true,
        (Some(grid), _) => grid.contains(&capability),
        (None, role) => role.can(capability),
    };
    from_role
        || (hr_access && capability.comes_with_hr_access())
        || (capability.grantable() && grants.contains(&capability))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WRITES: [Capability; 19] = {
        use Capability::*;
        [
            ManageUsers,
            ManageSettings,
            ManageConfiguration,
            OperateSystem,
            EditPartners,
            EditLeads,
            EditOrders,
            ChangeStages,
            ManageBlockers,
            UploadMedia,
            DeleteMedia,
            ViewOriginalImages,
            SendEmail,
            IssueInvoices,
            AnnulInvoices,
            AccessHr,
            Comment,
            ManageLeave,
            ManageRecruitment,
        ]
    };

    #[test]
    fn hr_access_brings_the_whole_module_and_its_parts_can_be_granted_alone() {
        use Capability::*;
        let none: &[Capability] = &[];
        for cap in [AccessHr, ManageLeave, ManageRecruitment] {
            assert!(holds(Role::Office, Some(none), true, none, cap), "{cap:?}");
            assert!(
                !holds(Role::Office, Some(none), false, none, cap),
                "{cap:?}"
            );
        }
        assert!(!holds(Role::Office, Some(none), true, none, ViewPrices));

        // A workshop head with leave only: not the directory, not the hiring.
        let leave = [ManageLeave];
        assert!(holds(
            Role::Designer,
            Some(none),
            false,
            &leave,
            ManageLeave
        ));
        assert!(!holds(Role::Designer, Some(none), false, &leave, AccessHr));
        assert!(!holds(
            Role::Designer,
            Some(none),
            false,
            &leave,
            ManageRecruitment
        ));

        // AccessHr is the flag's alone: a stored grant of it does nothing.
        assert!(!holds(
            Role::Office,
            Some(none),
            false,
            &[AccessHr],
            AccessHr
        ));
        assert!(ManageLeave.grantable() && ManageRecruitment.grantable());
    }

    #[test]
    fn viewer_cannot_write_anything_and_admin_has_everything() {
        for cap in WRITES {
            assert!(!Role::Viewer.can(cap), "viewer should not have {cap:?}");
        }
        for cap in Capability::ALL {
            assert!(Role::Admin.can(cap), "admin should have {cap:?}");
        }
    }

    #[test]
    fn capability_and_role_keys_round_trip() {
        for cap in Capability::ALL {
            assert_eq!(Capability::from_key(&cap.key()), Some(cap));
        }
        for role in Role::ALL {
            assert_eq!(Role::from_key(role.key()), Some(role));
            assert_eq!(serde_json::to_value(role).unwrap(), role.key());
        }
        assert_eq!(Capability::EditOrders.key(), "edit_orders");
        assert_eq!(Capability::ViewPrices.key(), "view_prices");
        assert!(!Capability::ManageUsers.grantable());
    }

    #[test]
    fn the_workshop_sees_the_job_but_not_the_money_or_the_customer() {
        use Capability::*;
        assert!(Role::Designer.can(UploadMedia));
        assert!(Role::Designer.can(ChangeStages));
        assert!(!Role::Designer.can(EditOrders));
        assert!(!Role::Designer.can(SendEmail));
        for cap in [
            ViewPrices,
            ViewInvoices,
            ViewIncomingInvoices,
            ViewSales,
            ViewContacts,
            ViewBusinessReports,
            ViewAllModules,
        ] {
            assert!(
                !Role::Designer.can(cap),
                "the workshop should not have {cap:?}"
            );
        }
    }

    #[test]
    fn sales_sells_and_the_accountant_invoices() {
        use Capability::*;
        assert!(Role::Sales.can(EditLeads) && Role::Sales.can(ViewPrices));
        assert!(!Role::Sales.can(ViewInvoices) && !Role::Sales.can(IssueInvoices));
        assert!(Role::Accountant.can(IssueInvoices) && Role::Accountant.can(ViewIncomingInvoices));
        assert!(!Role::Accountant.can(EditOrders) && !Role::Accountant.can(ViewSales));
    }

    #[test]
    fn the_shop_floor_and_the_purchaser_see_no_money() {
        use Capability::*;
        for role in [Role::Designer, Role::Worker, Role::Purchaser] {
            for cap in [ViewPrices, ViewInvoices, ViewSales, ViewBusinessReports] {
                assert!(!role.can(cap), "{role:?} should not have {cap:?}");
            }
        }
        assert!(Role::Purchaser.can(HandleHandovers) && Role::Purchaser.can(ViewContacts));
        assert!(!Role::Worker.can(HandleHandovers) && !Role::Worker.can(ChangeStages));
        assert!(Role::Worker.can(UploadMedia) && Role::Worker.can(Comment));
    }

    #[test]
    fn office_cannot_administer() {
        assert!(Role::Office.can(Capability::EditOrders));
        assert!(!Role::Office.can(Capability::ManageUsers));
        assert!(!Role::Office.can(Capability::ManageSettings));
    }
}
