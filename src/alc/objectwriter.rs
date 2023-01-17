use crate::tools::error::{FluteError, Result};
use std::{cell::RefCell, io::Write, rc::Rc};

///
/// Struct representing metadata for an object.
///
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// URI that can be used as an identifier for this object
    pub content_location: url::Url,
    /// Anticipated size of this object
    pub content_length: Option<usize>,
    /// The content type of this object.
    /// This field describes the format of the object's content,
    /// and can be used to determine how to handle or process the object.
    pub content_type: Option<String>,
}

///
/// Used to write Objects received by a FLUTE receiver to a destination
///
pub trait FluteWriter {
    /// Return a new object writer that will be used to store the received object to its final destination
    fn new_object_writer(&self, tsi: &u64, toi: &u128) -> Box<dyn ObjectWriter>;
}

///
/// Write s single object to its final destination
///
pub trait ObjectWriter {
    /// Open the destination
    fn open(&self, meta: Option<&ObjectMetadata>) -> Result<()>;
    /// Write data
    fn write(&self, data: &[u8]);
    /// Called when all the data has been written
    fn complete(&self);
    /// Called when an error occurred during the reception of this object
    fn error(&self);
}

///
/// Write objects received by the `receiver` to a buffers
///
#[derive(Debug)]
pub struct FluteWriterBuffer {
    /// List of all objects received
    pub objects: RefCell<Vec<Rc<RefCell<ObjectWriterBuffer>>>>,
}

///
/// Write a FLUTE object to a buffer
///
#[derive(Debug)]
struct ObjectWriterBufferWrapper {
    inner: Rc<RefCell<ObjectWriterBuffer>>,
}

#[derive(Debug)]
/// Object stored in a buffer
pub struct ObjectWriterBuffer {
    /// true when the object is fully received
    pub complete: bool,
    /// true when an error occured during the reception
    pub error: bool,
    /// buffer containing the data of the object
    pub data: Vec<u8>,
    /// Metadata of the object
    pub meta: Option<ObjectMetadata>,
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
    pub fn new() -> FluteWriterBuffer {
        FluteWriterBuffer {
            objects: RefCell::new(Vec::new()),
        }
    }
}

impl FluteWriter for FluteWriterBuffer {
    fn new_object_writer(&self, _tsi: &u64, _toi: &u128) -> Box<dyn ObjectWriter> {
        let obj = Rc::new(RefCell::new(ObjectWriterBuffer {
            complete: false,
            error: false,
            data: Vec::new(),
            meta: None,
        }));

        let obj_wrapper = Box::new(ObjectWriterBufferWrapper { inner: obj.clone() });
        self.objects.borrow_mut().push(obj);
        obj_wrapper
    }
}

impl ObjectWriter for ObjectWriterBufferWrapper {
    fn open(&self, meta: Option<&ObjectMetadata>) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.meta = meta.map(|meta| meta.clone());
        Ok(())
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        log::info!("Object complete !");
        inner.complete = true;
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        log::error!("Object received with error");
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
    pub fn new(dest: &std::path::Path) -> Result<FluteWriterFS> {
        if !dest.is_dir() {
            return Err(FluteError::new(format!("{:?} is not a directory", dest)));
        }

        Ok(FluteWriterFS {
            dest: dest.to_path_buf(),
        })
    }
}

impl FluteWriter for FluteWriterFS {
    fn new_object_writer(&self, _tsi: &u64, _toi: &u128) -> Box<dyn ObjectWriter> {
        let obj = Box::new(ObjectWriterFS {
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
/// Write an object to a file system
/// Uses the content-location to create the destination path of the object
/// If the destination path does not exists, the folder hierarchy is created
/// Existing files will be overwritten by this object
///
#[derive(Debug)]
pub struct ObjectWriterFS {
    /// Folder destination were the object will be written
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
    fn open(&self, meta: Option<&ObjectMetadata>) -> Result<()> {
        if meta.is_none() {
            return Ok(());
        }

        let meta = meta.unwrap();
        let content_location_path = meta.content_location.path();
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
