//! External systems: outbound email, the sales mailbox, the MNB exchange-rate feed, the NAV
//! invoicing sidecar, and the time-stamping authority.

pub mod email;
pub mod google;
pub mod imap;
pub mod mnb;
pub mod nav;
pub mod tsa;
pub mod vpic;
