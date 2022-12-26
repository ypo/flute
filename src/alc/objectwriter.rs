use crate::tools::error::{FluteError, Result};
use std::{cell::RefCell, io::Write, rc::Rc};

///
/// Used to write Objects received by a FLUTE receiver to a destination
///
pub trait FluteWriter {
    /// Return a new writer session for the object defined by its TSI and TOI
    fn create_session(&self, tsi: &u64, toi: &u128) -> Rc<dyn ObjectWriter>;
}

///
/// Write s single object to its final destination
///
pub trait ObjectWriter {
    /// Open the destination
    fn open(&self, content_location: Option<&url::Url>) -> Result<()>;
    /// Write data
    fn write(&self, data: &[u8]);
    /// Called when all the data has been written
    fn complete(&self);
    /// Called when an error occured during the reception of this object
    fn error(&self);
}

///
/// Write objects received by the `receiver` to a buffer
///
#[derive(Debug)]
pub struct FluteWriterBuffer {
    /// List of all objects received
    pub objects: RefCell<Vec<Rc<ObjectWriterBuffer>>>,
}

///
/// Writer session to write a single object to a buffer
///
#[derive(Debug)]
pub struct ObjectWriterBuffer {
    inner: RefCell<ObjectWriterBufferInner>,
}

#[derive(Debug)]
struct ObjectWriterBufferInner {
    complete: bool,
    error: bool,
    data: Vec<u8>,
    content_location: Option<String>,
}

impl std::fmt::Debug for dyn FluteWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriter {{  }}")
    }
}

impl std::fmt::Debug for dyn ObjectWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriter {{  }}")
    }
}

impl FluteWriterBuffer {
    /// Return a new `ObjectWriterBuffer`
    pub fn new() -> Rc<FluteWriterBuffer> {
        Rc::new(FluteWriterBuffer {
            objects: RefCell::new(Vec::new()),
        })
    }
}

impl FluteWriter for FluteWriterBuffer {
    fn create_session(&self, _tsi: &u64, _toi: &u128) -> Rc<dyn ObjectWriter> {
        let obj = Rc::new(ObjectWriterBuffer {
            inner: RefCell::new(ObjectWriterBufferInner {
                complete: false,
                error: false,
                data: Vec::new(),
                content_location: None,
            }),
        });
        let mut sessions = self.objects.borrow_mut();
        sessions.push(obj.clone());
        obj
    }
}

impl ObjectWriterBuffer {
    /// Get a copy of the buffer with the content of the received object
    pub fn data(&self) -> Vec<u8> {
        let inner = self.inner.borrow();
        inner.data.clone()
    }

    /// Get the Content-Location of the received object
    pub fn content_location(&self) -> Option<String> {
        let inner = self.inner.borrow();
        inner.content_location.clone()
    }
}

impl ObjectWriter for ObjectWriterBuffer {
    fn open(&self, content_location: Option<&url::Url>) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.content_location = content_location.map(|s| s.to_string());
        Ok(())
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.complete = true;
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.error = true;
    }
}

///
/// Write objects received by the `receiver` to a filesystem
///
#[derive(Debug)]
pub struct FluteWriterFS {
    dest: std::path::PathBuf,
}

impl FluteWriterFS {
    /// Return a new `ObjectWriterBuffer`
    pub fn new(dest: &std::path::Path) -> Result<Rc<FluteWriterFS>> {
        if !dest.is_dir() {
            return Err(FluteError::new(format!("{:?} is not a directory", dest)));
        }

        Ok(Rc::new(FluteWriterFS {
            dest: dest.to_path_buf(),
        }))
    }
}

impl FluteWriter for FluteWriterFS {
    fn create_session(&self, _tsi: &u64, _toi: &u128) -> Rc<dyn ObjectWriter> {
        let obj = Rc::new(ObjectWriterFS {
            dest: self.dest.clone(),
            inner: RefCell::new(ObjectWriterFSInner {
                destination: None,
                writer: None,
            }),
        });
        obj
    }
}

///
/// Write Objects to a file system
///
#[derive(Debug)]
pub struct ObjectWriterFS {
    dest: std::path::PathBuf,
    inner: RefCell<ObjectWriterFSInner>,
}

///
///
#[derive(Debug)]
pub struct ObjectWriterFSInner {
    destination: Option<std::path::PathBuf>,
    writer: Option<std::io::BufWriter<std::fs::File>>,
}

impl ObjectWriter for ObjectWriterFS {
    fn open(&self, content_location: Option<&url::Url>) -> Result<()> {
        if content_location.is_none() {
            return Ok(());
        }

        let content_location = content_location.unwrap();
        let content_location_path = content_location.path();
        let relative_path = content_location_path
            .strip_prefix("/")
            .unwrap_or_else(|| content_location_path);
        let destination = self.dest.join(relative_path);
        log::info!(
            "Create destination {:?} {:?} {:?}",
            self.dest,
            relative_path,
            destination
        );
        let parent = destination.parent();
        if parent.is_some() {
            let parent = parent.unwrap();
            if !parent.is_dir() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let file = std::fs::File::create(&destination)?;
        let mut inner = self.inner.borrow_mut();
        inner.writer = Some(std::io::BufWriter::new(file));
        inner.destination = Some(destination.to_path_buf());
        Ok(())
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        if inner.writer.is_none() {
            return;
        }
        match inner.writer.as_mut().unwrap().write_all(data) {
            Ok(_) => {}
            Err(e) => log::error!("Fail to write file {:?}", e),
        };
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.writer.is_none() {
            return;
        }

        log::info!("File {:?} is completed !", inner.destination);
        inner.writer.as_mut().unwrap().flush().ok();
        inner.writer = None;
        inner.destination = None
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.writer = None;
        if inner.destination.is_some() {
            log::error!("Remove file {:?}", inner.destination);
            std::fs::remove_file(inner.destination.as_ref().unwrap()).ok();
            inner.destination = None;
        }
    }
}
