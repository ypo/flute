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

mod fec;

/// FLUTE/ALC/LC protocol
pub mod alc;

/// UDP/IP
pub mod network;

/// Tools
pub mod tools;

#[cfg(test)]
mod tests {
    pub fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).try_init().ok();
    }
}
