use base64::Engine;

use super::objectdesc::{create_fdt_cache_control, ObjectDesc};
use super::FDTPublishMode;
use crate::common::oti::SchemeSpecific;
use crate::common::{fdtinstance, oti, partition};
use crate::error::{FluteError, Result};
use crate::sender::objectdesc::CarouselRepeatMode;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use std::time::SystemTime;

#[derive(Debug)]
struct TransferInfo {
    transferring: bool,
    transfer_count: u32,
    total_nb_transfer: u64,
    last_transfer_end_time: Option<SystemTime>,
    last_transfer_start_time: Option<SystemTime>,
    next_transfer_timestamp: Option<SystemTime>,
    packet_transmission_tick: Option<std::time::Duration>,
    transfer_start_time: Option<SystemTime>,
}

impl TransferInfo {
    fn init(&mut self, object: &ObjectDesc, oti: &oti::Oti, now: SystemTime) {
        self.transferring = true;
        self.last_transfer_start_time = Some(now);
        let mut packet_transmission_tick = None;
        if let Some(target_acquisition_latency) = object.config.target_acquisition.as_ref() {
            packet_transmission_tick = match target_acquisition_latency {
                crate::sender::objectdesc::TargetAcquisition::AsFastAsPossible => None,
                crate::sender::objectdesc::TargetAcquisition::WithinDuration(duration) => {
                    let nb_packets = object
                        .transfer_length
                        .div_ceil(oti.encoding_symbol_length as u64);
                    // TODO should we take into account the FEC encoding symbol length ?
                    Some(duration.div_f64(nb_packets as f64))
                }
                crate::sender::objectdesc::TargetAcquisition::WithinTime(target_time) => {
                    let duration = target_time.duration_since(now).unwrap_or_default();
                    if duration.is_zero() {
                        log::warn!(
                            "Target acquisition time is in the past target={:?} now={:?} for={}",
                            target_time,
                            now,
                            object.content_location
                        );
                    }
                    let nb_packets = object
                        .transfer_length
                        .div_ceil(oti.encoding_symbol_length as u64);
                    Some(duration.div_f64(nb_packets as f64))
                }
            }
        }

        self.packet_transmission_tick = packet_transmission_tick;
        if self.packet_transmission_tick.is_some() {
            self.next_transfer_timestamp = Some(now)
        }

        if self.transfer_count == object.config.max_transfer_count
            && object.config.carousel_mode.is_some()
        {
            self.transfer_count = 0;
        }
    }

    fn done(&mut self, now: SystemTime) {
        self.transferring = false;
        self.transfer_count += 1;
        self.total_nb_transfer += 1;
        self.last_transfer_end_time = Some(now);
    }

    fn tick(&mut self) {
        if let Some(tick) = self.packet_transmission_tick {
            if let Some(next_transfer_timestamp) = self.next_transfer_timestamp.as_mut() {
                if let Some(next) = next_transfer_timestamp.checked_add(tick) {
                    *next_transfer_timestamp = next;
                }
            }
        }
    }
}

/// Raptor systematic indices J(K) are only defined for 4 <= K <= 8192
/// <https://www.rfc-editor.org/rfc/rfc5053.html#section-5.7>
///
/// When the object is less than 4 symbols long, the symbol length is reduced to T' = floor(F / 4), rounded down to a multiple of Al
/// <https://www.rfc-editor.org/rfc/rfc5053.html#section-4.1>, so the object is encoded with at least 4 symbols.
fn raptor_encoding_symbol_length(
    transfer_length: u64,
    encoding_symbol_length: u16,
    symbol_alignment: u8,
) -> Result<u16> {
    if symbol_alignment == 0 {
        return Err(FluteError::new("Al must be at least 1"));
    }

    if encoding_symbol_length == 0 {
        return Ok(encoding_symbol_length);
    }

    let nb_symbols = num_integer::div_ceil(transfer_length, encoding_symbol_length as u64);
    if transfer_length == 0 || nb_symbols >= 4 {
        return Ok(encoding_symbol_length);
    }

    let al = symbol_alignment as u64;
    let symbol_length = transfer_length / 4 / al * al;
    if symbol_length == 0 {
        return Err(FluteError::new(format!(
            "Object transfer length of {} is too small to be encoded with Raptor, a minimum of {} bytes is required with Al={}",
            transfer_length,
            4 * al,
            al
        )));
    }

    Ok(symbol_length as u16)
}

#[derive(Debug)]
pub struct FileDesc {
    pub priority: u32,
    pub object: Box<ObjectDesc>,
    pub oti: oti::Oti,
    pub fdt_id: Option<u32>,
    pub sender_current_time: bool,
    pub published: AtomicBool,
    pub toi: u128,
    transfer_info: RwLock<TransferInfo>,
}

impl FileDesc {
    pub fn new(
        priority: u32,
        object: Box<ObjectDesc>,
        default_oti: &oti::Oti,
        fdt_id: Option<u32>,
        sender_current_time: bool,
    ) -> Result<FileDesc> {
        assert!(object.config.toi.is_some());
        let mut oti = match &object.config.oti {
            Some(res) => res.clone(),
            None => default_oti.clone(),
        };

        let max_transfer_length = oti.max_transfer_length();
        if object.transfer_length as usize > max_transfer_length {
            return Err(FluteError::new(format!(
                "Object transfer length of {} is bigger than {}, so is incompatible with the parameters of your OTI",
                object.transfer_length, max_transfer_length
            )));
        }

        if oti.fec_encoding_id == oti::FECEncodingID::Raptor {
            let scheme = match oti.scheme_specific.as_mut() {
                Some(SchemeSpecific::Raptor(scheme)) => scheme,
                _ => {
                    return Err(FluteError::new(
                        "FEC Raptor is selected, however scheme parameters are not defined",
                    ))
                }
            };

            oti.encoding_symbol_length = raptor_encoding_symbol_length(
                object.transfer_length,
                oti.encoding_symbol_length,
                scheme.symbol_alignment,
            )?;

            // A sub-symbol is at least Al bytes, so N can't be above T'/Al
            // <https://www.rfc-editor.org/rfc/rfc5053.html#section-4.2>
            let max_sub_blocks = oti.encoding_symbol_length / scheme.symbol_alignment as u16;
            if scheme.sub_blocks_length as u16 > max_sub_blocks {
                scheme.sub_blocks_length = max_sub_blocks as u8;
            }
        }

        if oti.fec_encoding_id == oti::FECEncodingID::RaptorQ
            || oti.fec_encoding_id == oti::FECEncodingID::Raptor
        {
            // Calculate the source block length of Raptor / RaptorQ

            let (a_large, a_small, nb_a_large, nb_blocks) = partition::block_partitioning(
                oti.maximum_source_block_length as u64,
                object.transfer_length,
                oti.encoding_symbol_length as u64,
            );

            if oti.fec_encoding_id == oti::FECEncodingID::RaptorQ {
                if oti.scheme_specific.is_none() {
                    return Err(FluteError::new(
                        "FEC RaptorQ is selected, however scheme parameters are not defined",
                    ));
                }

                // K'max <https://www.rfc-editor.org/rfc/rfc6330.html#section-5.1.2>
                if a_large > 56403 {
                    return Err(FluteError::new(format!(
                        "Object transfer length of {} is partitioned into source blocks of {} symbols, RaptorQ requires at most 56403 symbols per block, your object is incompatible with the FEC parameters of your OTI",
                        object.transfer_length, a_large
                    )));
                }

                let nb_blocks:u8 = nb_blocks.try_into().map_err(|_| {
                    FluteError::new(format!(
                        "Object transfer length of {} requires the transmission of {} source blocks, the maximum is {}, your object is incompatible with the FEC parameters of your OTI",
                        object.transfer_length,
                        nb_blocks, u8::MAX
                    ))
                })?;

                if let SchemeSpecific::RaptorQ(scheme) = oti.scheme_specific.as_mut().unwrap() {
                    scheme.source_blocks_length = nb_blocks;
                }
            } else if oti.fec_encoding_id == oti::FECEncodingID::Raptor {
                // A source block of 2 or 3 symbols is still possible, ex: a maximum source block length of 5
                let is_valid_block = |k: u64| (4..=8192).contains(&k);
                if (nb_a_large > 0 && !is_valid_block(a_large))
                    || (nb_blocks > nb_a_large && !is_valid_block(a_small))
                {
                    return Err(FluteError::new(format!(
                        "Object transfer length of {} is partitioned into source blocks of {}/{} symbols, Raptor requires 4 to 8192 symbols per block, your object is incompatible with the FEC parameters of your OTI",
                        object.transfer_length, a_large, a_small
                    )));
                }

                let nb_blocks:u16 = nb_blocks.try_into().map_err(|_| {
                    FluteError::new(format!(
                        "Object transfer length of {} requires the transmission of {} source blocks, the maximum is {}, your object is incompatible with the FEC parameters of your OTI",
                        object.transfer_length,
                        nb_blocks, u8::MAX
                    ))
                })?;

                if let SchemeSpecific::Raptor(scheme) = oti.scheme_specific.as_mut().unwrap() {
                    scheme.source_blocks_length = nb_blocks;
                }
            }
        }

        let toi = object.config.toi.as_ref().unwrap().get();
        let transfer_start_time = object.config.transfer_start_time.clone();
        Ok(FileDesc {
            priority,
            object,
            oti,
            fdt_id,
            sender_current_time,
            transfer_info: RwLock::new(TransferInfo {
                transferring: false,
                transfer_count: 0,
                last_transfer_start_time: None,
                last_transfer_end_time: None,
                total_nb_transfer: 0,
                next_transfer_timestamp: None,
                packet_transmission_tick: None,
                transfer_start_time,
            }),
            published: AtomicBool::new(false),
            toi,
        })
    }

    pub fn total_nb_transfer(&self) -> u64 {
        let info = self.transfer_info.read().unwrap();
        info.total_nb_transfer
    }

    pub fn can_transfer_be_stopped(&self) -> bool {
        if self
            .object
            .config
            .allow_immediate_stop_before_first_transfer
            == Some(true)
        {
            return true;
        }

        self.total_nb_transfer() > 0
    }

    pub fn transfer_started(&self, now: SystemTime) {
        let mut info = self.transfer_info.write().unwrap();
        info.init(&self.object, &self.oti, now);
    }

    pub fn transfer_done(&self, now: SystemTime) {
        let mut info = self.transfer_info.write().unwrap();
        info.done(now);
    }

    pub fn is_expired(&self) -> bool {
        let info = self.transfer_info.read().unwrap();
        if self.object.config.max_transfer_count > info.transfer_count {
            return false;
        }
        self.object.config.carousel_mode.is_none()
    }

    pub fn is_transferring(&self) -> bool {
        let info = self.transfer_info.read().unwrap();
        info.transferring
    }

    pub fn get_next_transfer_timestamp(&self) -> Option<SystemTime> {
        let info = self.transfer_info.read().unwrap();
        info.next_transfer_timestamp
    }

    pub fn inc_next_transfer_timestamp(&self) {
        let mut info = self.transfer_info.write().unwrap();
        info.tick();
    }

    pub fn reset_last_transfer(&self, start_time: Option<SystemTime>) {
        let mut info = self.transfer_info.write().unwrap();
        info.last_transfer_end_time = None;
        info.last_transfer_start_time = None;
        if start_time.is_some() {
            info.transfer_start_time = start_time;
        }
    }

    pub fn is_last_transfer(&self) -> bool {
        if self.object.config.carousel_mode.is_some() {
            return false;
        }

        let info = self.transfer_info.read().unwrap();
        self.object.config.max_transfer_count == info.transfer_count + 1
    }

    pub fn should_transfer_now(
        &self,
        priority: u32,
        fdt_publish_mode: FDTPublishMode,
        now: SystemTime,
    ) -> bool {
        if self.priority != priority {
            return false;
        }

        if fdt_publish_mode == FDTPublishMode::FullFDT && !self.is_published() {
            log::warn!("File with TOI {} is not published", self.toi);
            return false;
        }

        let info = self.transfer_info.read().unwrap();
        if let Some(start_time) = info.transfer_start_time {
            if now < start_time {
                return false;
            }
        }

        if info.transferring {
            return false;
        }

        if self.object.config.max_transfer_count > info.transfer_count {
            return true;
        }

        if self.object.config.carousel_mode.is_none()
            || info.last_transfer_end_time.is_none()
            || info.last_transfer_start_time.is_none()
        {
            return true;
        }

        let carousel_mode = self.object.config.carousel_mode.as_ref().unwrap();
        let (last_time, interval) = match carousel_mode {
            CarouselRepeatMode::DelayBetweenTransfers(interval) => {
                (info.last_transfer_end_time.as_ref().unwrap(), interval)
            }
            CarouselRepeatMode::IntervalBetweenStartTimes(interval) => {
                (info.last_transfer_start_time.as_ref().unwrap(), interval)
            }
        };

        let last_transfer_interval = now.duration_since(*last_time).unwrap_or_default();
        last_transfer_interval > *interval
    }

    pub fn is_published(&self) -> bool {
        self.published.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_published(&self) {
        self.published
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn to_file_xml(&self, now: SystemTime) -> fdtinstance::File {
        let oti_attributes = match self.oti.fec_encoding_id {
            // for RaptorQ and Raptor we need to add OTI for each object (Z, and T for Raptor, depend on the object)
            oti::FECEncodingID::RaptorQ | oti::FECEncodingID::Raptor => {
                Some(self.oti.get_attributes())
            }
            _ => self
                .object
                .config
                .oti
                .as_ref()
                .map(|oti| oti.get_attributes()),
        };

        let optel_propagator = self
            .object
            .config
            .optel_propagator
            .as_ref()
            .map(|propagator| {
                let s = serde_json::to_string(&propagator).unwrap();
                base64::engine::general_purpose::STANDARD.encode(s)
            });

        fdtinstance::File {
            content_location: self.object.content_location.to_string(),
            toi: self.toi.to_string(),
            content_length: Some(self.object.content_length),
            transfer_length: Some(self.object.transfer_length),
            content_type: Some(self.object.content_type.clone()),
            content_encoding: match &self.object.config.cenc {
                crate::core::lct::Cenc::Null => None,
                _ => Some(self.object.config.cenc.to_str().to_string()),
            },
            content_md5: self.object.md5.clone(),
            fec_oti_fec_encoding_id: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_fec_encoding_id),
            fec_oti_fec_instance_id: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_fec_instance_id),
            fec_oti_maximum_source_block_length: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_maximum_source_block_length),
            fec_oti_encoding_symbol_length: oti_attributes
                .as_ref()
                .map(|f| f.fec_oti_encoding_symbol_length)
                .unwrap_or_default(),
            fec_oti_max_number_of_encoding_symbols: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_max_number_of_encoding_symbols),
            fec_oti_scheme_specific_info: oti_attributes
                .and_then(|f| f.fec_oti_scheme_specific_info),
            cache_control: self
                .object
                .config
                .cache_control
                .as_ref()
                .map(|cc| create_fdt_cache_control(cc, now)),
            alternate_content_location_1: None,
            alternate_content_location_2: None,
            mbms_session_identity: None,
            decryption_key_uri: None,
            fec_redundancy_level: None,
            file_etag: self.object.config.e_tag.clone(),
            independent_unit_positions: None,
            delimiter: Some(0),
            delimiter2: Some(0),
            group: None,
            optel_propagator,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{raptor_encoding_symbol_length, FileDesc};
    use crate::common::oti::{FECEncodingID, Oti, RaptorSchemeSpecific, SchemeSpecific};
    use crate::error::Result;
    use crate::sender::objectdesc::{ObjectDesc, TransferConfig};
    use crate::sender::toiallocator::ToiAllocator;
    use crate::sender::TOIMaxLength;

    fn create_file_desc(transfer_length: usize, default_oti: &Oti) -> Result<FileDesc> {
        let allocator = ToiAllocator::new(TOIMaxLength::ToiMax112, None);
        let mut obj = ObjectDesc::create_from_buffer(
            vec![0u8; transfer_length],
            "application/octet-stream",
            &url::Url::parse("file:///object").unwrap(),
            false,
            TransferConfig::default(),
        )
        .unwrap();
        obj.set_toi(ToiAllocator::allocate(&allocator));
        FileDesc::new(0, obj, default_oti, None, false)
    }

    fn get_raptor_scheme(oti: &Oti) -> &RaptorSchemeSpecific {
        match oti.scheme_specific.as_ref() {
            Some(SchemeSpecific::Raptor(scheme)) => scheme,
            _ => panic!("Raptor scheme specific is missing"),
        }
    }

    #[test]
    pub fn test_raptor_encoding_symbol_length() {
        crate::tests::init();

        // (F, T, Al, T')
        let cases = [
            (0u64, 1024u16, 4u8, 1024u16),
            (16, 1024, 4, 4),
            (1024, 1024, 4, 256),
            (1025, 1024, 4, 256),
            (2048, 1024, 4, 512),
            (2049, 1024, 4, 512),
            (3072, 1024, 4, 768),
            (3073, 1024, 4, 1024),
            (100000, 1024, 4, 1024),
            (1100, 1024, 8, 272),
            (1100, 1023, 3, 273),
            (1100, 1024, 1, 275),
            (16, 8, 4, 4),
        ];
        for (f, t, al, expected) in cases {
            assert_eq!(
                raptor_encoding_symbol_length(f, t, al).unwrap(),
                expected,
                "F={} T={} Al={}",
                f,
                t,
                al
            );
        }

        // F < 4 * Al
        assert!(raptor_encoding_symbol_length(1, 1024, 4).is_err());
        assert!(raptor_encoding_symbol_length(12, 8, 4).is_err());
        assert!(raptor_encoding_symbol_length(5, 4, 4).is_err());
        assert!(raptor_encoding_symbol_length(1025, 1024, 0).is_err());
    }

    #[test]
    pub fn test_raptor_encoding_symbol_length_min_4_symbols() {
        crate::tests::init();
        for (t, al) in [
            (4u16, 4u8),
            (8, 4),
            (16, 4),
            (64, 1),
            (1023, 3),
            (1024, 4),
            (1024, 8),
        ] {
            for f in (4 * al as u64)..=(4 * t as u64) {
                let symbol_length = raptor_encoding_symbol_length(f, t, al).unwrap();
                let k = num_integer::div_ceil(f, symbol_length as u64);
                assert_eq!(symbol_length % al as u16, 0, "F={} T={} Al={}", f, t, al);
                assert!(k >= 4, "F={} T={} Al={} K={}", f, t, al, k);
            }
        }
    }

    #[test]
    pub fn test_file_desc_raptor() {
        crate::tests::init();
        let oti = Oti::new_raptor(1024, 64, 20, 1, 4).unwrap();

        // (F, T', Z)
        let cases = [
            (16usize, 4u16, 1u16),
            (1024, 256, 1),
            (1025, 256, 1),
            (2048, 512, 1),
            (2049, 512, 1),
            (3072, 768, 1),
            (3073, 1024, 1),
            (100000, 1024, 2),
        ];
        for (transfer_length, t, z) in cases {
            let file = create_file_desc(transfer_length, &oti).unwrap();
            assert_eq!(file.oti.encoding_symbol_length, t);
            assert_eq!(get_raptor_scheme(&file.oti).source_blocks_length, z);

            // OTI of the File entry in the FDT
            let file_oti = file
                .to_file_xml(std::time::SystemTime::now())
                .get_oti()
                .unwrap();
            assert_eq!(file_oti.fec_encoding_id, FECEncodingID::Raptor);
            assert_eq!(file_oti.encoding_symbol_length, t);
            assert_eq!(get_raptor_scheme(&file_oti).source_blocks_length, z);
        }
    }

    #[test]
    pub fn test_file_desc_raptor_sub_blocks() {
        crate::tests::init();
        let oti = Oti::new_raptor(1024, 64, 20, 8, 4).unwrap();

        // (F, T', N)
        let cases = [
            (100000usize, 1024u16, 8u8),
            (1025, 256, 8),
            (100, 24, 6),
            (16, 4, 1),
        ];
        for (transfer_length, t, n) in cases {
            let file = create_file_desc(transfer_length, &oti).unwrap();
            assert_eq!(file.oti.encoding_symbol_length, t);
            assert_eq!(get_raptor_scheme(&file.oti).sub_blocks_length, n);

            let file_oti = file
                .to_file_xml(std::time::SystemTime::now())
                .get_oti()
                .unwrap();
            assert_eq!(get_raptor_scheme(&file_oti).sub_blocks_length, n);
        }
    }

    #[test]
    pub fn test_file_desc_raptor_error() {
        crate::tests::init();

        // F < 4 * Al
        let oti = Oti::new_raptor(8, 64, 2, 1, 4).unwrap();
        assert!(create_file_desc(12, &oti).is_err());
        assert!(create_file_desc(16, &oti).is_ok());
        let oti = Oti::new_raptor(1024, 64, 2, 1, 4).unwrap();
        assert!(create_file_desc(1, &oti).is_err());

        // Source blocks of 2 or 3 symbols
        let oti = Oti::new_raptor(1024, 4, 2, 1, 4).unwrap();
        assert!(create_file_desc(4 * 1024, &oti).is_ok()); // 4
        assert!(create_file_desc(5 * 1024, &oti).is_err()); // 3 + 2
        assert!(create_file_desc(8 * 1024, &oti).is_ok()); // 4 + 4
        assert!(create_file_desc(1025, &oti).is_err()); // T'=256 -> 3 + 2
        let oti = Oti::new_raptor(1024, 5, 2, 1, 4).unwrap();
        assert!(create_file_desc(6 * 1024, &oti).is_err()); // 3 + 3
        let oti = Oti::new_raptor(1024, 6, 2, 1, 4).unwrap();
        assert!(create_file_desc(7 * 1024, &oti).is_err()); // 4 + 3

        // Source block above Kmax (OTI not created with new_raptor)
        let mut oti = Oti::new_raptor(16, 64, 20, 1, 4).unwrap();
        oti.maximum_source_block_length = 10000;
        assert!(create_file_desc(9000 * 16, &oti).is_err());

        let mut oti = Oti::new_raptor(1024, 64, 20, 1, 4).unwrap();
        if let Some(SchemeSpecific::Raptor(scheme)) = oti.scheme_specific.as_mut() {
            scheme.symbol_alignment = 0;
        }
        assert!(create_file_desc(1025, &oti).is_err());

        let mut oti = Oti::new_raptor(1024, 64, 20, 1, 4).unwrap();
        oti.scheme_specific = None;
        assert!(create_file_desc(1025, &oti).is_err());
    }

    #[test]
    pub fn test_file_desc_other_fec_unchanged() {
        crate::tests::init();

        let oti = Oti::new_no_code(1024, 64);
        let file = create_file_desc(1025, &oti).unwrap();
        assert_eq!(file.oti.encoding_symbol_length, 1024);
        assert!(file
            .to_file_xml(std::time::SystemTime::now())
            .fec_oti_encoding_symbol_length
            .is_none());

        let oti = Oti::new_raptorq(1024, 64, 20, 1, 4).unwrap();
        let file = create_file_desc(1025, &oti).unwrap();
        assert_eq!(file.oti.encoding_symbol_length, 1024);
    }
}
