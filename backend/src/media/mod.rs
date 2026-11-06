//! Media: file storage (an object store or a mounted share), where each file is filed, upload
//! tickets, the image processing pipeline, drawing thumbnails, HEIC conversion and ZIP
//! downloads.

pub mod cad;
pub mod fs_store;
pub mod heic;
pub mod layout;
pub mod pipeline;
pub mod storage;
pub mod upload_token;
pub mod zip;
