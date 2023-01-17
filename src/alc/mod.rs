mod alc;
mod alccodec;
mod block;
mod blockdecoder;
mod blockencoder;
mod blockwriter;
mod compress;
mod fdt;
mod fdtinstance;
mod fdtreceiver;
mod filedesc;
mod objectreceiver;
mod pkt;
mod sendersession;
mod uncompress;
mod partition;

pub mod lct;
pub mod multireceiver;
pub mod objectdesc;
pub mod oti;
pub mod receiver;
pub mod sender;

///
/// Write FLUTE objects to their final destination
///
/// # Example
///
/// ```
/// use flute::receiver::objectwriter;
///
/// let writer = objectwriter::FluteWriterFS::new(&std::path::Path::new("./destination_dir")).ok();
/// ```
///
pub mod objectwriter;
