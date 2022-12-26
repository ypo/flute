use crate::tools::error::{FluteError, Result};

/// Content Encoding, compressed
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CENC {
    /// Do not encode content before transmission
    Null = 0,
    /// Encode content with ZLIB
    Zlib = 1,
    /// Encode content with Deflate
    Deflate = 2,
    /// Encode content with Gzip
    Gzip = 3,
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum EXT {
    Fdt = 192,
    Fti = 64,
    Cenc = 193,
    Time = 2,
}

pub const TOI_FDT: u128 = 0;

#[derive(Clone, Debug)]
pub struct LCTHeader {
    pub len: usize,
    pub cci: u128,
    pub tsi: u64,
    pub toi: u128,
    pub cp: u8,
    pub close_object: bool,
    pub close_session: bool,
    pub header_ext_offset: u32,
    pub length: usize,
}

impl TryFrom<u8> for CENC {
    type Error = ();

    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            x if x == CENC::Null as u8 => Ok(CENC::Null),
            x if x == CENC::Zlib as u8 => Ok(CENC::Zlib),
            x if x == CENC::Deflate as u8 => Ok(CENC::Deflate),
            x if x == CENC::Gzip as u8 => Ok(CENC::Gzip),
            _ => Err(()),
        }
    }
}

impl TryFrom<&str> for CENC {
    type Error = ();

    fn try_from(v: &str) -> std::result::Result<Self, Self::Error> {
        match v {
            "null" => Ok(CENC::Null),
            "zlib" => Ok(CENC::Zlib),
            "deflate" => Ok(CENC::Deflate),
            "gzip" => Ok(CENC::Gzip),
            _ => Err(()),
        }
    }
}

impl CENC {
    /// Convert CENC to its string representation
    pub fn to_str(&self) -> &str {
        match self {
            CENC::Null => "null",
            CENC::Zlib => "zlib",
            CENC::Deflate => "deflate",
            CENC::Gzip => "gzip",
        }
    }
}

fn nb_bytes_128(cci: &u128) -> u32 {
    if (cci & 0xFFFF0000000000000000000000000000) != 0x0 {
        return 16;
    }

    if (cci & 0xFFFF000000000000000000000000) != 0x0 {
        return 14;
    }

    if (cci & 0xFFFF00000000000000000000) != 0x0 {
        return 12;
    }

    if (cci & 0xFFFF0000000000000000) != 0x0 {
        return 10;
    }

    if (cci & 0xFFFF000000000000) != 0x0 {
        return 8;
    }

    if (cci & 0xFFFF00000000) != 0x0 {
        return 6;
    }

    if (cci & 0xFFFF0000) != 0x0 {
        return 4;
    }

    if (cci & 0xFFFF) != 0x0 {
        return 2;
    }

    return 0;
}

fn nb_bytes_64(n: u64) -> u32 {
    if (n & 0xFFFF000000000000) != 0x0 {
        return 8;
    }

    if (n & 0xFFFF00000000) != 0x0 {
        return 6;
    }

    if (n & 0xFFFF0000) != 0x0 {
        return 4;
    }

    if (n & 0xFFFF) != 0x0 {
        return 2;
    }

    return 0;
}

/**
 *  https://www.rfc-editor.org/rfc/rfc5651
 *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 *  |   V   | C |PSI|S| O |H|Res|A|B|   HDR_LEN     | Codepoint (CP)|
 *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 *  | Congestion Control Information (CCI, length = 32*(C+1) bits)  |
 *  |                          ...                                  |
 *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 *  |  Transport Session Identifier (TSI, length = 32*S+16*H bits)  |
 *  |                          ...                                  |
 *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 *  |   Transport Object Identifier (TOI, length = 32*O+16*H bits)  |
 *  |                          ...                                  |
 *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 *  |                Header Extensions (if applicable)              |
 *  |                          ...                                  |
 *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 *  
 * LCT version number (V): 4 bits
 * Congestion control flag (C): 2 bits
 *      C=0 indicates the Congestion Control Information (CCI) field is 32 bits in length.  
 *      C=1 indicates the CCI field is 64 bits in length.
 *      C=2 indicates the CCI field is 96 bits in length.  
 *      C=3 indicates the CCI field is 128 bits in length.
 * Protocol-Specific Indication (PSI): 2 bits
 *     The usage of these bits, if any, is specific to each protocol instantiation that uses the LCT building block.  
 *      If no protocol instantiation-specific usage of these bits is defined, then a sender MUST set them to zero and a receiver MUST ignore these bits.
 * Transport Session Identifier flag (S): 1 bit
 *     This is the number of full 32-bit words in the TSI field.
 *     The TSI field is 32*S + 16*H bits in length, i.e., the length is either 0 bits, 16 bits, 32 bits, or 48 bits.
 * Transport Object Identifier flag (O): 2 bits
 *      This is the number of full 32-bit words in the TOI field.  The TOI
 *      field is 32*O + 16*H bits in length, i.e., the length is either 0
 *      bits, 16 bits, 32 bits, 48 bits, 64 bits, 80 bits, 96 bits, or 112
 *      bits.
 * Half-word flag (H): 1 bit
 *      The TSI and the TOI fields are both multiples of 32 bits plus 16*H
 *      bits in length.  This allows the TSI and TOI field lengths to be
 *      multiples of a half-word (16 bits), while ensuring that the
 *      aggregate length of the TSI and TOI fields is a multiple of 32
 *      bits.
 * Reserved (Res): 2 bits
 *      These bits are reserved.  In this version of the specification,
 *      they MUST be set to zero by senders and MUST be ignored by
 *      receivers.
 * Close Session flag (A): 1 bit
 *      Normally, A is set to 0.  The sender MAY set A to 1 when termination of transmission of packets for the session is
 *      imminent.  A MAY be set to 1 in just the last packet transmitted for the session, or A MAY be set to 1 in the last few seconds of
 *      packets transmitted for the session.  Once the sender sets A to 1 in one packet, the sender SHOULD set A to 1 in all subsequent
 *      packets until termination of transmission of packets for the session.  
 *      A received packet with A set to 1 indicates to a receiver that the sender will immediately stop sending packets for the session.  
 *      When a receiver receives a packet with A set to 1, the receiver SHOULD assume that no more packets will be sent to the session.
 * Close Object flag (B): 1 bit
 *      Normally, B is set to 0.  The sender MAY set B to 1 when termination of transmission of packets for an object is imminent.
 *      If the TOI field is in use and B is set to 1, then termination of transmission for the object identified by the TOI field is
 *      imminent.  If the TOI field is not in use and B is set to 1, then termination of transmission for the one object in the session
 *      identified by out-of-band information is imminent.  B MAY be set to 1 in just the last packet transmitted for the object, or B MAY
 *      be set to 1 in the last few seconds that packets are transmitted for the object.  Once the sender sets B to 1 in one packet for a
 *      particular object, the sender SHOULD set B to 1 in all subsequent packets for the object until termination of transmission of
 *      packets for the object.  A received packet with B set to 1 indicates to a receiver that the sender will immediately stop
 *      sending packets for the object.  When a receiver receives a packet with B set to 1, then it SHOULD assume that no more packets will
 *      be sent for the object to the session.
 * LCT header length (HDR_LEN): 8 bits
 *      Total length of the LCT header in units of 32-bit words.  The
 *      length of the LCT header MUST be a multiple of 32 bits.  This
 *      field can be used to directly access the portion of the packet
 *      beyond the LCT header, i.e., to the first other header if it
 *      exists, or to the packet payload if it exists and there is no
 *      other header, or to the end of the packet if there are no other
 *      headers or packet payload.
 * Codepoint (CP): 8 bits
 *      An opaque identifier that is passed to the packet payload decoder
 *      to convey information on the codec being used for the packet
 *      payload.  The mapping between the codepoint and the actual codec
 *      is defined on a per session basis and communicated out-of-band as
 *      part of the session description information.  The use of the CP
 *      field is similar to the Payload Type (PT) field in RTP headers as
 *      described in [RFC3550].
 */
pub fn push_lct_header(
    data: &mut Vec<u8>,
    psi: u8,
    cci: &u128,
    tsi: u64,
    toi: &u128,
    codepoint: u8,
    close_object: bool,
) {
    let cci_size = nb_bytes_128(cci);
    let tsi_size = nb_bytes_64(tsi);
    let toi_size = nb_bytes_128(toi);

    let h_tsi = (tsi_size & 2) >> 1; // Is TSI half-word ?
    let h_toi = (toi_size & 2) >> 1; // Is TOI half-word ?

    let h = h_tsi | h_toi; // Half-word flag
    let b: u8 = match close_object {
        true => 1,
        false => 0,
    };
    let a: u8 = 0;
    let o = (toi_size >> 2) & 0x3;
    let s = (tsi_size >> 2) & 1;
    let c = match cci_size {
        size if size <= 4 => 0,
        size if size <= 8 => 1,
        size if size <= 12 => 2,
        _ => 3,
    };
    let hdr_len: u8 = (2 + o + s + h + c) as u8;
    let v = 1;
    let lct_header: u32 = (codepoint as u32)
        | ((hdr_len as u32) << 8)
        | (b as u32) << 16
        | (a as u32) << 17
        | (h as u32) << 20
        | (o as u32) << 21
        | (s as u32) << 23
        | (psi as u32) << 24
        | (c as u32) << 26
        | (v as u32) << 28;

    data.extend(lct_header.to_be_bytes());

    // Insert CCI
    let cci_net = cci.to_be_bytes();
    let cci_net_start: usize = cci_net.len() - ((c + 1) << 2) as usize;
    data.extend(&cci_net[cci_net_start..]);

    // Insert TSI
    let tsi_net = tsi.to_be_bytes();
    let tsi_net_start = tsi_net.len() - ((s << 2) + (h << 1)) as usize;
    data.extend(&tsi_net[tsi_net_start..]);

    // Insert TOI
    let toi_net = toi.to_be_bytes();
    let toi_net_start = toi_net.len() - ((o << 2) + (h << 1)) as usize;
    data.extend(&toi_net[toi_net_start..]);
}

pub fn inc_hdr_len(data: &mut Vec<u8>, val: u8) {
    data[2] += val;
}

pub fn parse_lct_header(data: &[u8]) -> Result<LCTHeader> {
    /*
     *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     *  |   V   | C |PSI|S| O |H|Res|A|B|   HDR_LEN     | Codepoint (CP)|
     *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     *  | Congestion Control Information (CCI, length = 32*(C+1) bits)  |
     *  |                          ...                                  |
     *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     *  |  Transport Session Identifier (TSI, length = 32*S+16*H bits)  |
     *  |                          ...                                  |
     *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     *  |   Transport Object Identifier (TOI, length = 32*O+16*H bits)  |
     *  |                          ...                                  |
     *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     *  |                Header Extensions (if applicable)              |
     *  |                          ...                                  |
     *  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */

    let len = data.get(2).map_or_else(
        || Err(FluteError::new("Fail to read lct header size")),
        |&v| Ok((v as usize) << 2),
    )?;

    if len > data.len() {
        return Err(FluteError::new(format!(
            "lct header size is {} whereas pkt size is {}",
            len,
            data.len()
        )));
    }

    let cp = data[3];
    let flags1 = data[0];
    let flags2 = data[1];

    let s = (flags2 >> 7) & 0x1;
    let o = (flags2 >> 5) & 0x3;
    let h = (flags2 >> 4) & 0x1;
    let c = (flags1 >> 2) & 0x3;
    let b = (flags2 << 1) & 0x1;
    let a = flags2 & 0x1;
    let version = flags1 >> 4;
    if version != 1 && version != 2 {
        return Err(FluteError::new(format!(
            "FLUTE version {} is not supported",
            version
        )));
    }

    let cci_len = ((c + 1) as u32) << 2;
    let tsi_len = ((s as u32) << 2) + ((h as u32) << 1);
    let toi_len = ((o as u32) << 2) + ((h as u32) << 1);

    let cci_from: usize = 4;
    let cci_to: usize = (4 + cci_len) as usize;
    let tsi_to: usize = cci_to + tsi_len as usize;
    let toi_to: usize = tsi_to + toi_len as usize;
    let header_ext_offset = toi_to as u32;

    if toi_to > data.len() || cci_len > 16 || tsi_len > 8 || toi_len > 16 {
        return Err(FluteError::new(format!(
            "toi ends to offset {} whereas pkt size is {}",
            toi_to,
            data.len()
        )));
    }

    if header_ext_offset > len as u32 {
        return Err(FluteError::new(format!("EXT offset outside LCT header",)));
    }

    let mut cci: [u8; 16] = [0; 16]; // Store up to 128 bits
    let mut tsi: [u8; 8] = [0; 8]; // Store up to 64 bits
    let mut toi: [u8; 16] = [0; 16]; // Store up to 128 bits

    let _ = &cci[(16 - cci_len) as usize..].copy_from_slice(&data[cci_from..cci_to]);
    let _ = &tsi[(8 - tsi_len) as usize..].copy_from_slice(&data[cci_to..tsi_to]);
    let _ = &toi[(16 - toi_len) as usize..].copy_from_slice(&data[tsi_to..toi_to]);

    let cci = u128::from_be_bytes(cci);
    let tsi = u64::from_be_bytes(tsi);
    let toi = u128::from_be_bytes(toi);

    Ok(LCTHeader {
        len,
        cci,
        tsi,
        toi,
        cp,
        close_object: b != 0,
        close_session: a != 0,
        header_ext_offset,
        length: len,
    })
}

pub fn get_ext<'a>(data: &'a [u8], lct: &LCTHeader, ext: EXT) -> Result<Option<&'a [u8]>> {
    let mut lct_ext_ext = &data[(lct.header_ext_offset as usize)..lct.len];
    while lct_ext_ext.len() > 4 {
        let het = lct_ext_ext[0];
        let hel = match het {
            het if het >= 128 => 4 as usize,
            _ => (lct_ext_ext[1] << 2) as usize,
        };

        if hel == 0 || hel > lct_ext_ext.len() {
            return Err(FluteError::new(format!(
                "Fail, LCT EXT size is {}/{} het={} offset={}",
                hel,
                lct_ext_ext.len(),
                het,
                lct.header_ext_offset
            )));
        }

        if het == ext as u8 {
            return Ok(Some(&lct_ext_ext[..hel]));
        }
        lct_ext_ext = &lct_ext_ext[hel..];
    }

    Ok(None)
}

#[cfg(test)]
mod tests {

    #[test]
    pub fn test_lct() {
        crate::tests::init();
        let mut lct = Vec::new();
        let psi: u8 = 0;
        let cci: u128 = 0x1;
        let tsi: u64 = 0;
        let toi: u128 = 0;
        let codepoint: u8 = 0;
        super::push_lct_header(&mut lct, psi, &cci, tsi, &toi, codepoint, false)
    }
}
