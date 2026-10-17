use super::{ObjectMetadata, ObjectWriter, ObjectWriterBuilder, ObjectWriterBuilderResult};
use crate::{
    common::udpendpoint::UDPEndpoint,
    error::{FluteError, Result},
};
use std::{cell::RefCell, io::Write, time::SystemTime};

///
/// Write objects received by the `receiver` to a filesystem
///
#[derive(Debug)]
pub struct ObjectWriterFSBuilder {
    dest: std::path::PathBuf,
    enable_md5_check: bool,
}

impl ObjectWriterFSBuilder {
    /// Return a new `ObjectWriterBuffer`
    pub fn new(dest: &std::path::Path, enable_md5_check: bool) -> Result<ObjectWriterFSBuilder> {
        if !dest.is_dir() {
            return Err(FluteError::new(format!("{:?} is not a directory", dest)));
        }

        Ok(ObjectWriterFSBuilder {
            dest: dest.to_path_buf(),
            enable_md5_check,
        })
    }
}

impl ObjectWriterBuilder for ObjectWriterFSBuilder {
    fn new_object_writer(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        meta: &ObjectMetadata,
        _now: std::time::SystemTime,
    ) -> ObjectWriterBuilderResult {
        ObjectWriterBuilderResult::StoreObject(Box::new(ObjectWriterFS {
            dest: self.dest.clone(),
            inner: RefCell::new(ObjectWriterFSInner {
                destination: None,
                writer: None,
            }),
            meta: meta.clone(),
            enable_md5_check: self.enable_md5_check,
        }))
    }

    fn update_cache_control(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        _meta: &ObjectMetadata,
        _now: std::time::SystemTime,
    ) {
    }

    fn fdt_received(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _fdt_xml: &str,
        _expires: std::time::SystemTime,
        _meta: &ObjectMetadata,
        _transfer_duration: std::time::Duration,
        _now: std::time::SystemTime,
        _ext_time: Option<std::time::SystemTime>,
    ) {
    }
}

///
/// Write an object to a file system.  
/// Uses the content-location to create the destination path of the object.  
/// If the destination path does not exists, the folder hierarchy is created.  
/// Existing files will be overwritten by this object.
///
#[derive(Debug)]
pub struct ObjectWriterFS {
    /// Folder destination were the object will be written
    dest: std::path::PathBuf,
    inner: RefCell<ObjectWriterFSInner>,
    meta: ObjectMetadata,
    enable_md5_check: bool,
}

///
///
#[derive(Debug)]
pub struct ObjectWriterFSInner {
    destination: Option<std::path::PathBuf>,
    writer: Option<std::io::BufWriter<std::fs::File>>,
}

impl ObjectWriter for ObjectWriterFS {
    fn open(&self, _now: SystemTime) -> Result<()> {
        let url = url::Url::parse(&self.meta.content_location);
        let content_location_path = match &url {
            Ok(url) => url.path(),
            Err(e) => match e {
                url::ParseError::RelativeUrlWithoutBase => &self.meta.content_location,
                url::ParseError::RelativeUrlWithCannotBeABaseBase => &self.meta.content_location,
                _ => {
                    log::error!(
                        "Fail to parse content location {:?} {:?}",
                        self.meta.content_location,
                        e
                    );
                    return Err(FluteError::new(format!(
                        "Fail to parse content location {:?} {:?}",
                        self.meta.content_location, e
                    )));
                }
            },
        };
        let relative_path = sanitize_relative_path(content_location_path)?;
        let destination = self.dest.join(&relative_path);
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

    fn write(&self, _sbn: u32, data: &[u8], _now: SystemTime) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        if inner.writer.is_none() {
            return Ok(());
        }
        inner
            .writer
            .as_mut()
            .unwrap()
            .write_all(data)
            .map_err(|e| {
                log::error!("Fail to write data to file {:?} {:?}", inner.destination, e);
                FluteError::new(format!(
                    "Fail to write data to file {:?} {:?}",
                    inner.destination, e
                ))
            })?;
        Ok(())
    }

    fn complete(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        if inner.writer.is_none() {
            return;
        }

        println!("File {:?} is completed !", inner.destination);
        inner.writer.as_mut().unwrap().flush().ok();
        inner.writer = None;
        inner.destination = None
    }

    fn error(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        inner.writer = None;
        if inner.destination.is_some() {
            log::error!("Remove file {:?}", inner.destination);
            std::fs::remove_file(inner.destination.as_ref().unwrap()).ok();
            inner.destination = None;
        }
    }

    fn interrupted(&self, now: SystemTime) {
        self.error(now);
    }

    fn enable_md5_check(&self) -> bool {
        self.enable_md5_check
    }
}

/// Convert the path of a Content-Location (received from the network) into a
/// relative path that cannot escape the destination directory.
fn sanitize_relative_path(content_location_path: &str) -> Result<std::path::PathBuf> {
    let mut relative_path = std::path::PathBuf::new();
    for component in
        std::path::Path::new(content_location_path.trim_start_matches('/')).components()
    {
        match component {
            std::path::Component::Normal(part) => relative_path.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                log::error!(
                    "Content location {:?} escapes the destination directory",
                    content_location_path
                );
                return Err(FluteError::new(format!(
                    "Content location {:?} escapes the destination directory",
                    content_location_path
                )));
            }
        }
    }

    if relative_path.as_os_str().is_empty() {
        return Err(FluteError::new(format!(
            "Content location {:?} does not contain a file name",
            content_location_path
        )));
    }

    Ok(relative_path)
}

#[cfg(test)]
mod tests {
    use super::sanitize_relative_path;
    use std::path::PathBuf;

    #[test]
    fn test_sanitize_relative_path() {
        crate::tests::init();
        assert_eq!(
            sanitize_relative_path("/dir/file.txt").unwrap(),
            PathBuf::from("dir/file.txt")
        );
        assert_eq!(
            sanitize_relative_path("//tmp/./file.txt").unwrap(),
            PathBuf::from("tmp/file.txt")
        );
        assert_eq!(
            sanitize_relative_path("file.txt").unwrap(),
            PathBuf::from("file.txt")
        );

        for path in [
            "../file.txt",
            "/dir/../../file.txt",
            "dir/..",
            "",
            "/",
            "./",
        ] {
            assert!(sanitize_relative_path(path).is_err(), "{:?}", path);
        }
    }

    #[test]
    fn test_open_outside_destination() {
        use crate::common::udpendpoint::UDPEndpoint;
        use crate::receiver::writer::{
            ObjectCacheControl, ObjectMetadata, ObjectWriterBuilder, ObjectWriterBuilderResult,
        };
        use std::time::SystemTime;

        crate::tests::init();
        let root = std::env::temp_dir().join(format!("flute_fs_{}", std::process::id()));
        let dest = root.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        let builder = super::ObjectWriterFSBuilder::new(&dest, false).unwrap();
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 3400);
        let escaped = root.join("escaped.txt");
        let locations = [
            "../escaped.txt".to_owned(),
            format!("/{}", escaped.display()),
            "file:///../escaped.txt".to_owned(),
        ];

        for location in &locations {
            let meta = ObjectMetadata {
                content_location: location.clone(),
                content_length: None,
                transfer_length: None,
                content_type: None,
                cache_control: ObjectCacheControl::NoCache,
                groups: None,
                md5: None,
                optel_propagator: None,
                oti: None,
                cenc: None,
                e_tag: None,
            };
            let writer =
                match builder.new_object_writer(&endpoint, &1, &1, &meta, SystemTime::now()) {
                    ObjectWriterBuilderResult::StoreObject(writer) => writer,
                    _ => panic!("unexpected writer result"),
                };
            if writer.open(SystemTime::now()).is_ok() {
                writer.complete(SystemTime::now());
            }
            assert!(!escaped.exists(), "{:?} escaped the destination", location);
        }

        std::fs::remove_dir_all(&root).ok();
    }
}
