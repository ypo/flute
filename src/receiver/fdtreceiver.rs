use super::objectreceiver;
use super::writer::{ObjectWriterBuilder, ObjectWriterBuilderResult};
use crate::common::udpendpoint::UDPEndpoint;
use crate::common::{alc, fdtinstance::FdtInstance, lct};
use crate::{receiver::writer::ObjectMetadata, tools};
use crate::{receiver::writer::ObjectWriter, tools::error::Result};
use std::{cell::RefCell, rc::Rc, time::SystemTime};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FDTState {
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
    pub ext_time: Option<std::time::SystemTime>,
    pub reception_start_time: SystemTime,
    enable_expired_check: bool,
    meta: Option<ObjectMetadata>,
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
            .field("receiver_start_time", &self.reception_start_time)
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
    state: FDTState,
}

impl FdtReceiver {
    pub fn new(
        endpoint: &UDPEndpoint,
        tsi: u64,
        fdt_id: u32,
        enable_expired_check: bool,
        now: SystemTime,
    ) -> FdtReceiver {
        let inner = Rc::new(RefCell::new(FdtWriterInner {
            data: Vec::new(),
            fdt: None,
            state: FDTState::Receiving,
            expires: None,
        }));

        let fdt_builder = Rc::new(FdtWriterBuilder::new(inner.clone()));

        FdtReceiver {
            fdt_id,
            obj: Some(Box::new(objectreceiver::ObjectReceiver::new(
                endpoint,
                tsi,
                &lct::TOI_FDT,
                Some(fdt_id),
                fdt_builder,
                1024 * 1024,
                now,
            ))),
            inner: inner.clone(),
            fdt_instance: None,
            sender_current_time_offset: None,
            sender_current_time_late: true,
            reception_start_time: now,
            enable_expired_check,
            meta: None,
            ext_time: None,
        }
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt, now: std::time::SystemTime) {
        if let Ok(Some(res)) = alc::get_sender_current_time(pkt) {
            self.ext_time = Some(res);
            if res < now {
                self.sender_current_time_late = true;
                self.sender_current_time_offset = Some(now.duration_since(res).unwrap())
            } else {
                self.sender_current_time_late = false;
                self.sender_current_time_offset = Some(res.duration_since(now).unwrap())
            }
        }

        if let Some(obj) = self.obj.as_mut() {
            obj.push(pkt, now);
            match obj.state {
                objectreceiver::State::Receiving => {}
                objectreceiver::State::Completed => {
                    self.meta = Some(obj.create_meta());
                    self.obj = None
                }
                objectreceiver::State::Interrupted => {
                    self.inner.borrow_mut().state = FDTState::Error
                }
                objectreceiver::State::Error => self.inner.borrow_mut().state = FDTState::Error,
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

    pub fn state(&self) -> FDTState {
        self.inner.borrow().state
    }

    pub fn fdt_instance(&mut self) -> Option<&FdtInstance> {
        if self.fdt_instance.is_none() {
            let inner = self.inner.borrow();
            let instance = inner.fdt.as_ref();
            self.fdt_instance = instance.cloned();
        }
        self.fdt_instance.as_ref()
    }

    pub fn fdt_xml_str(&self) -> Option<String> {
        let inner = self.inner.borrow();
        String::from_utf8(inner.data.clone()).ok()
    }

    pub fn fdt_meta(&self) -> Option<&ObjectMetadata> {
        self.meta.as_ref()
    }

    pub fn update_expired_state(&self, now: SystemTime) {
        if self.state() != FDTState::Complete {
            return;
        }

        if self.enable_expired_check && self.is_expired(now) {
            let mut inner = self.inner.borrow_mut();
            inner.state = FDTState::Expired;
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

    pub fn get_expiration_time(&self) -> Option<SystemTime> {
        let inner = self.inner.borrow();
        inner.expires
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
        _meta: &ObjectMetadata,
        _now: std::time::SystemTime,
    ) -> ObjectWriterBuilderResult {
        ObjectWriterBuilderResult::StoreObject(Box::new(FdtWriter {
            inner: self.inner.clone(),
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
        __meta: &ObjectMetadata,
        _transfer_duration: std::time::Duration,
        _now: std::time::SystemTime,
        _ext_time: Option<std::time::SystemTime>,
    ) {
    }
}

impl ObjectWriter for FdtWriter {
    fn open(&self, _now: SystemTime) -> Result<()> {
        Ok(())
    }

    fn write(&self, _sbn: u32, data: &[u8], _now: SystemTime) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
        Ok(())
    }

    fn complete(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        match FdtInstance::parse(&inner.data) {
            Ok(inst) => {
                inner.expires = match inst.expires.parse::<u32>() {
                    Ok(seconds_ntp) => tools::ntp_to_system_time((seconds_ntp as u64) << 32).ok(),
                    _ => None,
                };
                inner.fdt = Some(inst);
                inner.state = FDTState::Complete
            }
            Err(_) => inner.state = FDTState::Error,
        };
    }

    fn error(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        inner.state = FDTState::Error;
    }

    fn interrupted(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        inner.state = FDTState::Error;
    }

    fn enable_md5_check(&self) -> bool {
        false
    }
}
