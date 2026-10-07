//! Four roles, explicit capabilities, checked at the handler boundary.
//! No policy DSL: adding a capability forces a decision for every role (exhaustive match).

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Office,
    Designer,
    Viewer,
}

/// Every authenticated user may read orders, partners, leads, images and reports.
/// Capabilities cover everything beyond reading. The role gives a default set; an admin can
/// grant any [`Capability::grantable`] one to a single user on top of it (`users.permissions`).
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
    /// The HR module: the staff directory with personal phone numbers. Admins always have
    /// it; anyone else needs the per-user `hr_access` flag an admin sets, which is checked
    /// in `AuthUser`, because a role alone cannot say it.
    AccessHr,
    /// Write comments on orders and leads. The shop floor talks about jobs too, so
    /// designers may; viewers read only.
    Comment,
}

impl Capability {
    pub const ALL: [Capability; 17] = {
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
        ]
    };

    /// What an admin may grant one user beyond their role. Not `ManageUsers`: whoever has
    /// it can grant themselves everything else, so it is the admin role itself. Not
    /// `AccessHr` either: that has its own `hr_access` flag.
    pub fn grantable(self) -> bool {
        !matches!(self, Capability::ManageUsers | Capability::AccessHr)
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
    pub fn can(self, capability: Capability) -> bool {
        use Capability::*;
        use Role::*;
        match capability {
            ManageUsers | ManageSettings | ManageConfiguration | OperateSystem | AnnulInvoices
            | AccessHr => {
                matches!(self, Admin)
            }
            EditPartners | EditLeads | EditOrders | DeleteMedia | ViewOriginalImages
            | SendEmail | IssueInvoices => {
                matches!(self, Admin | Office)
            }
            ChangeStages | ManageBlockers | UploadMedia | Comment => {
                matches!(self, Admin | Office | Designer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_cannot_write_anything() {
        use Capability::*;
        for cap in [
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
        ] {
            assert!(!Role::Viewer.can(cap), "viewer should not have {cap:?}");
            assert!(Role::Admin.can(cap), "admin should have {cap:?}");
        }
    }

    #[test]
    fn capability_keys_round_trip() {
        for cap in Capability::ALL {
            assert_eq!(Capability::from_key(&cap.key()), Some(cap));
        }
        assert_eq!(Capability::EditOrders.key(), "edit_orders");
        assert!(!Capability::ManageUsers.grantable());
    }

    #[test]
    fn designer_works_on_production_but_not_commercial_data() {
        assert!(Role::Designer.can(Capability::UploadMedia));
        assert!(Role::Designer.can(Capability::ChangeStages));
        assert!(!Role::Designer.can(Capability::EditOrders));
        assert!(!Role::Designer.can(Capability::SendEmail));
    }

    #[test]
    fn office_cannot_administer() {
        assert!(Role::Office.can(Capability::EditOrders));
        assert!(!Role::Office.can(Capability::ManageUsers));
        assert!(!Role::Office.can(Capability::ManageSettings));
    }
}
