mod alccodec;

/// FLUTE Profile
#[derive(Debug, Copy, Clone)]
pub enum Profile {
    /// FLUTE Version 2  
    /// <https://www.rfc-editor.org/rfc/rfc6726.html>
    RFC6726,
    /// FLUTE Version 1
    /// <https://www.rfc-editor.org/rfc/rfc3926>
    RFC3926,
}

pub mod alc;
pub mod fdtinstance;
pub(crate) mod fdtinstance_l6;
pub mod lct;
pub mod oti;
pub mod partition;
pub mod pkt;
pub mod udpendpoint;
