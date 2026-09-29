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
/// Capabilities cover everything beyond reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

impl Role {
    pub fn can(self, capability: Capability) -> bool {
        use Capability::*;
        use Role::*;
        match capability {
            ManageUsers | ManageSettings | ManageConfiguration | OperateSystem | AnnulInvoices => {
                matches!(self, Admin)
            }
            EditPartners | EditLeads | EditOrders | DeleteMedia | ViewOriginalImages
            | SendEmail | IssueInvoices => {
                matches!(self, Admin | Office)
            }
            ChangeStages | ManageBlockers | UploadMedia => {
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
        ] {
            assert!(!Role::Viewer.can(cap), "viewer should not have {cap:?}");
            assert!(Role::Admin.can(cap), "admin should have {cap:?}");
        }
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
