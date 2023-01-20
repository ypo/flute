use super::{alccodec::AlcCodec, lct, oti, pkt::Pkt};
use crate::tools::{self, error::FluteError, error::Result};
use std::time::SystemTime;

#[derive(Debug)]
pub struct AlcPkt<'a> {
    pub lct: lct::LCTHeader,
    pub oti: Option<oti::Oti>,
    pub transfer_length: Option<u64>,
    pub cenc: Option<lct::Cenc>,
    pub server_time: Option<SystemTime>,
    pub data: &'a [u8],
    pub data_alc_header_offset: usize,
    pub data_payload_offset: usize,
    pub fdt_info: Option<ExtFDT>,
}

#[derive(Debug)]
pub struct AlcPktCache {
    pub lct: lct::LCTHeader,
    pub oti: Option<oti::Oti>,
    pub transfer_length: Option<u64>,
    pub cenc: Option<lct::Cenc>,
    pub server_time: Option<SystemTime>,
    pub data_alc_header_offset: usize,
    pub data_payload_offset: usize,
    pub data: Vec<u8>,
    pub fdt_info: Option<ExtFDT>,
}

pub struct PayloadID {
    pub sbn: u32,
    pub esi: u32,
    pub source_block_length: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ExtFDT {
    pub version: u32,
    pub fdt_instance_id: u32,
}

impl<'a> AlcPkt<'a> {
    pub fn to_cache(&self) -> AlcPktCache {
        AlcPktCache {
            lct: self.lct.clone(),
            oti: self.oti.clone(),
            transfer_length: self.transfer_length,
            cenc: self.cenc.clone(),
            server_time: self.server_time.clone(),
            data_alc_header_offset: self.data_alc_header_offset,
            data_payload_offset: self.data_payload_offset,
            data: self.data.to_vec(),
            fdt_info: self.fdt_info.clone(),
        }
    }
}

impl<'a> AlcPktCache {
    pub fn to_pkt(&'a self) -> AlcPkt<'a> {
        AlcPkt {
            lct: self.lct.clone(),
            oti: self.oti.clone(),
            transfer_length: self.transfer_length,
            cenc: self.cenc.clone(),
            server_time: self.server_time.clone(),
            data_alc_header_offset: self.data_alc_header_offset,
            data_payload_offset: self.data_payload_offset,
            data: self.data.as_ref(),
            fdt_info: self.fdt_info.clone(),
        }
    }
}

pub fn new_alc_pkt(oti: &oti::Oti, cci: &u128, tsi: u64, pkt: &Pkt) -> Vec<u8> {
    let mut data = Vec::new();
    log::debug!("Send ALC sbn={} esi={} toi={}", pkt.sbn, pkt.esi, pkt.toi);
    lct::push_lct_header(
        &mut data,
        0,
        &cci,
        tsi,
        &pkt.toi,
        oti.fec_encoding_id as u8,
        pkt.close_object,
    );

    if pkt.toi == lct::TOI_FDT {
        assert!(pkt.fdt_id.is_some());
        push_fdt(&mut data, 2, pkt.fdt_id.unwrap())
    }

    // In case of FDT, we must push Cenc if Cenc is not null
    if (pkt.toi == lct::TOI_FDT && (pkt.cenc != lct::Cenc::Null)) || pkt.inband_cenc {
        push_cenc(&mut data, pkt.cenc as u8);
    }

    if pkt.sender_current_time.is_some() {
        push_sct(&mut data, pkt.sender_current_time.unwrap());
    }

    let codec = <dyn AlcCodec>::instance(oti.fec_encoding_id);
    if pkt.toi == lct::TOI_FDT || oti.inband_fti {
        codec.add_fti(&mut data, oti, pkt.transfer_length);
    }
    codec.add_fec_payload_id(&mut data, oti, pkt);
    push_payload(&mut data, pkt);
    data
}

pub fn parse_alc_pkt(data: &[u8]) -> Result<AlcPkt> {
    let lct_header = lct::parse_lct_header(data)?;

    let fec: oti::FECEncodingID = lct_header
        .cp
        .try_into()
        .map_err(|_| FluteError::new(format!("Codepoint {} not supported", lct_header.cp)))?;

    let codec = <dyn AlcCodec>::instance(fec);
    let fec_payload_id_block_length = codec.fec_payload_id_block_length();
    if fec_payload_id_block_length + lct_header.len > data.len() {
        return Err(FluteError::new("Wrong size of ALC packet"));
    }

    let fti = codec.get_fti(data, &lct_header)?;
    let data_alc_header_offset = lct_header.len;
    let data_payload_offset = fec_payload_id_block_length + lct_header.len;

    let cenc = lct::get_ext(data.as_ref(), &lct_header, lct::Ext::Cenc)?;
    let cenc = match cenc {
        Some(ext) => parse_cenc(ext).ok(),
        None => None,
    };

    let mut fdt_info: Option<ExtFDT> = None;
    if lct_header.toi == lct::TOI_FDT {
        let fdt = lct::get_ext(data.as_ref(), &lct_header, lct::Ext::Fdt)?;
        fdt_info = match fdt {
            Some(ext) => parse_ext_fdt(ext)?,
            None => None,
        };
    }

    Ok(AlcPkt {
        lct: lct_header,
        oti: fti.as_ref().map(|fti| fti.0.clone()),
        transfer_length: fti.map(|fti| fti.1),
        cenc: cenc,
        server_time: None,
        data: data.as_ref(),
        data_alc_header_offset,
        data_payload_offset,
        fdt_info,
    })
}

pub fn get_sender_current_time(pkt: &AlcPkt) -> Result<Option<SystemTime>> {
    let ext = match lct::get_ext(pkt.data, &pkt.lct, lct::Ext::Time)? {
        Some(res) => res,
        _ => return Ok(None),
    };

    parse_sct(ext)
}

pub fn parse_payload_id(pkt: &AlcPkt, oti: &oti::Oti) -> Result<PayloadID> {
    let codec = <dyn AlcCodec>::instance(oti.fec_encoding_id);
    codec.get_fec_payload_id(pkt, oti)
}

fn parse_ext_fdt(ext: &[u8]) -> Result<Option<ExtFDT>> {
    if ext.len() != 4 {
        return Err(FluteError::new("Wrong size of FDT Extension"));
    }

    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |   HET = 192   |   V   |          FDT Instance ID              |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */

    let fdt_bytes = u32::from_be_bytes(ext.try_into().unwrap());
    let version = (fdt_bytes >> 20) & 0xF;
    let fdt_instance_id = fdt_bytes & 0xFFFF;

    Ok(Some(ExtFDT {
        version,
        fdt_instance_id,
    }))
}

fn push_fdt(data: &mut Vec<u8>, version: u8, fdt_id: u32) {
    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |   HET = 192   |   V   |          FDT Instance ID              |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */
    let ext = (lct::Ext::Fdt as u32) << 24 | (version as u32) << 20 | fdt_id;
    data.extend(ext.to_be_bytes());
    lct::inc_hdr_len(data, 1);
}

fn push_cenc(data: &mut Vec<u8>, cenc: u8) {
    /*
     0                   1                   2                   3
     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |   HET = 193   |     Cenc      |          Reserved             |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
      */
    let ext = (lct::Ext::Cenc as u32) << 24 | (cenc as u32) << 16;
    data.extend(ext.to_be_bytes());
    lct::inc_hdr_len(data, 1);
}

fn parse_cenc(ext: &[u8]) -> Result<lct::Cenc> {
    if ext.len() != 4 {
        return Err(FluteError::new("Wrong extension size"));
    }
    ext[1]
        .try_into()
        .map_err(|_| FluteError::new("Cenc not supported"))
}

fn push_sct(data: &mut Vec<u8>, time: std::time::SystemTime) {
    /*
     0                   1                   2                   3
     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |     HET = 2   |    HEL >= 1   |         Use (bit field)       |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |                       first time value                        |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    ...            (other time values (optional)                  ...
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+   */

    /*
     Use (bit field)                                       3
      6   7   8   9   0   1   2   3   4   5   6   7   8   9   0   1
    +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
    |SCT|SCT|ERT|SLC|   reserved    |          PI-specific          |
    |Hi |Low|   |   |    by LCT     |              use              |
    +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
    */

    /* HEL | SCT HIGH | SCT LOW */
    let header: u32 = (lct::Ext::Time as u32) << 24 | (3u32 << 16) | (1u32 << 15) | (1u32 << 14);

    // Convert UTC to NTP
    let ntp = match tools::system_time_to_ntp(time) {
        Ok(res) => res,
        Err(_) => return,
    };
    data.extend(header.to_be_bytes());
    data.extend(ntp.to_be_bytes());
    lct::inc_hdr_len(data, 3);
}

fn parse_sct(ext: &[u8]) -> Result<Option<std::time::SystemTime>> {
    assert!(ext.len() >= 4);
    let use_bits_hi = ext[2];
    let sct_hi = (use_bits_hi >> 7) & 1;
    let sct_low = (use_bits_hi >> 6) & 1;
    let ert = (use_bits_hi >> 5) & 1;
    let slc = (use_bits_hi >> 4) & 1;

    let expected_len = (sct_hi + sct_low + ert + slc + 1) as usize * 4;
    if ext.len() != expected_len {
        return Err(FluteError::new(format!(
            "Wrong ext length, expect {} received {}",
            expected_len,
            ext.len()
        )));
    }

    if sct_hi == 0 {
        return Ok(None);
    }

    let ntp_seconds: u32 = u32::from_be_bytes(ext[4..8].as_ref().try_into().unwrap());
    let ntp_faction: u32 = match sct_low {
        1 => u32::from_be_bytes(ext[8..12].as_ref().try_into().unwrap()),
        _ => 0,
    };
    let ntp: u64 = ((ntp_seconds as u64) << 32) | (ntp_faction as u64);
    tools::ntp_to_system_time(ntp).map(|op| Some(op))
}

fn push_payload(data: &mut Vec<u8>, pkt: &Pkt) {
    data.extend(pkt.payload.iter());
}

#[cfg(test)]
mod tests {
    use crate::common::lct;
    use crate::common::oti;
    use crate::common::pkt;

    #[test]
    pub fn test_alc_create() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let cci: u128 = 0x804754755879;
        let tsi: u64 = 0x055789451234;
        let payload = vec!['h' as u8, 'e' as u8, 'l' as u8, 'l' as u8, 'o' as u8];
        let transfer_length = payload.len() as u64;

        let pkt = pkt::Pkt {
            payload: payload,
            esi: 1,
            sbn: 2,
            toi: 3,
            fdt_id: None,
            cenc: lct::Cenc::Null,
            inband_cenc: true,
            transfer_length: transfer_length,
            close_object: false,
            source_block_length: 1,
            sender_current_time: None,
        };

        let alc_pkt = super::new_alc_pkt(&oti, &cci, tsi, &pkt);
        let decoded_pkt = super::parse_alc_pkt(&alc_pkt).unwrap();
        assert!(decoded_pkt.lct.toi == pkt.toi);
        assert!(decoded_pkt.lct.cci == cci);
        assert!(decoded_pkt.lct.tsi == tsi);
    }
}
