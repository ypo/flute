use super::objectreceiver;
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
    obj: Box<objectreceiver::ObjectReceiver>,
    inner: Rc<RefCell<FdtWriterInner>>,
    fdt_instance: Option<FdtInstance>,
    sender_current_time: Option<SystemTime>,
    receiver_current_time: SystemTime,
}

impl std::fmt::Debug for FdtReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FdtReceiver")
            .field("fdt_id", &self.fdt_id)
            .field("obj", &self.obj)
            .field("inner", &self.inner)
            .field("fdt_instance", &self.fdt_instance)
            .field("sender_current_time", &self.sender_current_time)
            .field("receiver_current_time", &self.receiver_current_time)
            .finish()
    }
}

#[derive(Debug)]
struct FdtWriter {
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
    pub fn new(fdt_id: u32, now: SystemTime) -> FdtReceiver {
        let inner = Rc::new(RefCell::new(FdtWriterInner {
            data: Vec::new(),
            fdt: None,
            state: State::Receiving,
            expires: None,
        }));

        let writer = Box::new(FdtWriter {
            inner: inner.clone(),
        });

        FdtReceiver {
            fdt_id,
            obj: Box::new(objectreceiver::ObjectReceiver::new(&lct::TOI_FDT, writer)),
            inner: inner.clone(),
            fdt_instance: None,
            sender_current_time: None,
            receiver_current_time: now,
        }
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt) {
        if self.sender_current_time.is_none() {
            match alc::get_sender_current_time(pkt) {
                Ok(res) => self.sender_current_time = res,
                _ => {}
            }
        }

        self.obj.push(pkt);
        if self.obj.state == objectreceiver::State::Error {
            self.inner.borrow_mut().state = State::Error;
        }
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
            log::info!("FDT is expired");
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

        if self.sender_current_time.is_some() {
            let expires_duration = match expires.duration_since(self.sender_current_time.unwrap()) {
                Ok(res) => res,
                _ => return true,
            };
            return self
                .receiver_current_time
                .duration_since(now)
                .unwrap_or_default()
                > expires_duration;
        }

        now > expires
    }
}

impl ObjectWriter for FdtWriter {
    fn open(&self, _meta: Option<&ObjectMetadata>) -> Result<()> {
        Ok(())
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data)
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
