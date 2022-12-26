//! # FLUTE - File Delivery over Unidirectional Transport
//!
//! Implementation of the FLUTE protocol in pure RUST
//!
//! # RFC
//!
//! | RFC      | Title      | Link       |
//! | ------------- | ------------- | ------------- |
//! | RFC 6726 | FLUTE - File Delivery over Unidirectional Transport | <https://www.rfc-editor.org/rfc/rfc6726.html> |
//! | RFC 5775 | Asynchronous Layered Coding (ALC) Protocol Instantiation | <https://www.rfc-editor.org/rfc/rfc5775.html> |
//! | RFC 5052 | Forward Error Correction (FEC) Building Block | <https://www.rfc-editor.org/rfc/rfc5052> |
//! | RFC 5510 | Reed-Solomon Forward Error Correction (FEC) Schemes | <https://www.rfc-editor.org/rfc/rfc5510.html> |
//!

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

mod alc;
mod fec;
mod tools;

///
/// FLUTE Receivers to re-construct ALC/LCT packets to Objects (files)
///
pub mod receiver {
    pub use crate::alc::multireceiver::MultiReceiver;
    pub use crate::alc::objectwriter;
    pub use crate::alc::receiver::Config;
    pub use crate::alc::receiver::Receiver;
}

/// FLUTE Sender to convert Objects (files) to ALC/LCT packets
pub mod sender {
    pub use crate::alc::lct::CENC;
    pub use crate::alc::objectdesc::ObjectDesc;
    pub use crate::alc::oti::FECEncodingID;
    pub use crate::alc::oti::Oti;
    pub use crate::alc::sender::Sender;
}

pub use crate::tools::error;

#[cfg(test)]
mod tests {
    pub fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).try_init().ok();
    }
}
