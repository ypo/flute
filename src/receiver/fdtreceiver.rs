use super::writer::ObjectWriterBuilder;
use super::{objectreceiver, UDPEndpoint};
use crate::common::{alc, fdtinstance::FdtInstance, lct};
use crate::{receiver::writer::ObjectMetadata, tools};
use crate::{receiver::writer::ObjectWriter, tools::error::Result};
use std::{cell::RefCell, rc::Rc, time::SystemTime};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Receiving,
    Complete,
    Error,
    Expired,
}

pub struct FdtReceiver {
    pub fdt_id: u32,
    obj: Option<Box<objectreceiver::ObjectReceiver>>,
    inner: Rc<RefCell<FdtWriterInner>>,
    fdt_instance: Option<FdtInstance>,
    sender_current_time_offset: Option<std::time::Duration>,
    sender_current_time_late: bool,
    receiver_current_time: SystemTime,
}

impl std::fmt::Debug for FdtReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FdtReceiver")
            .field("fdt_id", &self.fdt_id)
            .field("obj", &self.obj)
            .field("inner", &self.inner)
            .field("fdt_instance", &self.fdt_instance)
            .field(
                "sender_current_time_offset",
                &self.sender_current_time_offset,
            )
            .field("sender_current_time_late", &self.sender_current_time_late)
            .field("receiver_current_time", &self.receiver_current_time)
            .finish()
    }
}

#[derive(Debug)]
struct FdtWriter {
    inner: Rc<RefCell<FdtWriterInner>>,
}

struct FdtWriterBuilder {
    inner: Rc<RefCell<FdtWriterInner>>,
}

#[derive(Debug)]
struct FdtWriterInner {
    data: Vec<u8>,
    fdt: Option<FdtInstance>,
    expires: Option<SystemTime>,
    state: State,
}

impl FdtReceiver {
    pub fn new(endpoint: &UDPEndpoint, tsi: u64, fdt_id: u32, now: SystemTime) -> FdtReceiver {
        let inner = Rc::new(RefCell::new(FdtWriterInner {
            data: Vec::new(),
            fdt: None,
            state: State::Receiving,
            expires: None,
        }));

        let fdt_builder = Rc::new(FdtWriterBuilder::new(inner.clone()));

        FdtReceiver {
            fdt_id,
            obj: Some(Box::new(objectreceiver::ObjectReceiver::new(
                endpoint,
                tsi,
                &lct::TOI_FDT,
                fdt_builder,
            ))),
            inner: inner.clone(),
            fdt_instance: None,
            sender_current_time_offset: None,
            sender_current_time_late: true,
            receiver_current_time: now,
        }
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt, now: std::time::SystemTime) {
        match alc::get_sender_current_time(pkt) {
            Ok(Some(res)) => {
                if res < now {
                    self.sender_current_time_late = true;
                    self.sender_current_time_offset = Some(now.duration_since(res).unwrap())
                } else {
                    self.sender_current_time_late = false;
                    self.sender_current_time_offset = Some(res.duration_since(now).unwrap())
                }
            }
            _ => {}
        }

        if let Some(obj) = self.obj.as_mut() {
            obj.push(pkt, now);
            match obj.state {
                objectreceiver::State::Receiving => {}
                objectreceiver::State::Completed => self.obj = None,
                objectreceiver::State::Error => self.inner.borrow_mut().state = State::Error,
            }
        }
    }

    pub fn get_server_time(&self, now: std::time::SystemTime) -> std::time::SystemTime {
        if let Some(offset) = self.sender_current_time_offset {
            if self.sender_current_time_late {
                return now - offset;
            } else {
                return now + offset;
            }
        }

        now
    }

    pub fn state(&self) -> State {
        self.inner.borrow().state
    }

    pub fn fdt_instance(&mut self) -> Option<&FdtInstance> {
        if self.fdt_instance.is_none() {
            let inner = self.inner.borrow();
            let instance = inner.fdt.as_ref();
            self.fdt_instance = instance.map(|f| f.clone())
        }
        self.fdt_instance.as_ref()
    }

    pub fn update_expired_state(&self, now: SystemTime) {
        if self.state() != State::Complete {
            return;
        }

        if self.is_expired(now) {
            let mut inner = self.inner.borrow_mut();
            inner.state = State::Expired;
        }
    }

    fn is_expired(&self, now: SystemTime) -> bool {
        let inner = self.inner.borrow();
        let expires = match inner.expires {
            Some(expires) => expires,
            None => return true,
        };

        self.get_server_time(now) > expires
    }
}

impl FdtWriterBuilder {
    fn new(inner: Rc<RefCell<FdtWriterInner>>) -> Self {
        FdtWriterBuilder { inner }
    }
}

impl ObjectWriterBuilder for FdtWriterBuilder {
    fn new_object_writer(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        _meta: Option<&ObjectMetadata>,
    ) -> Box<dyn ObjectWriter> {
        Box::new(FdtWriter {
            inner: self.inner.clone(),
        })
    }

    fn set_cache_duration(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        _content_location: &url::Url,
        _duration: &std::time::Duration,
    ) {
    }
}

impl ObjectWriter for FdtWriter {
    fn open(&self) -> Result<()> {
        Ok(())
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        match FdtInstance::parse(&inner.data) {
            Ok(inst) => {
                inner.expires = match inst.expires.parse::<u32>() {
                    Ok(seconds_ntp) => tools::ntp_to_system_time((seconds_ntp as u64) << 32).ok(),
                    _ => None,
                };
                inner.fdt = Some(inst);
                inner.state = State::Complete
            }
            Err(_) => inner.state = State::Error,
        };
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.state = State::Error;
    }
}
