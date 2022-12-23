/// Generic FLUTE Error
#[derive(Debug)]
pub struct FluteError(pub std::io::Error);

///
pub type Result<T> = std::result::Result<T, FluteError>;

impl FluteError {
    /// Return a new FLUTE Error with a message
    pub fn new<E>(msg: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>> + std::fmt::Debug,
    {
        log::error!("{:?}", msg);
        FluteError(std::io::Error::new(std::io::ErrorKind::Other, msg))
    }

    /// Return a new FLUTE Error
    pub fn new_kind<E>(kind: std::io::ErrorKind, msg: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>> + std::fmt::Debug,
    {
        log::error!("{:?}", msg);
        FluteError(std::io::Error::new(kind, msg))
    }
}

impl From<std::io::Error> for FluteError {
    fn from(err: std::io::Error) -> Self {
        log::error!("{:?}", err);
        FluteError(err)
    }
}
