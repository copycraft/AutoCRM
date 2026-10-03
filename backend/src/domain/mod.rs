//! Pure types and business rules. No async, no database, no network.
//!
//! Types here may derive `sqlx::Type` / serde so they can cross the boundary without
//! mirror types, but nothing in this module performs IO. Every rule is unit-testable
//! without a database.

pub mod blocker;
pub mod email;
pub mod invoice;
pub mod lookups;
pub mod media;
pub mod money;
pub mod order;
pub mod partner;
pub mod role;
pub mod stage;
pub mod template;
