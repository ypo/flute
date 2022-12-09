use super::{lct, oti::Oti, pkt::Pkt};

struct BlockID {
    snb: u32,
    esi: u32,
}

struct PayloadID {
    snb: u32,
    esi: u32,
    snbLength: u32,
}

struct Alc {
    data: Vec<u8>,
}

impl Alc {
    pub fn create(oti: &Oti, cci: &u128, tsi: u64, pkt: &Pkt, add_sct: bool, now: u64) -> Alc {
        let mut alc = Alc { data: vec![0] };
        lct::push_lct_header(&mut alc.data, 0, &cci, tsi, &pkt.toi, oti.fec as u8);

        if pkt.toi == lct::toi_fdt {
            alc.push_fdt(1, pkt.fdt_id)
        }

        if pkt.cenc != lct::CENC::Null && pkt.inband_cenc {
            alc.push_cenc(pkt.cenc as u8);
        }

        if add_sct {
            alc.push_sct(now);
        }

        alc
    }

    fn push_fdt(&mut self, version: u8, fdt_id: u32) {
        /*
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |   HET = 192   |   V   |          FDT Instance ID              |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */
        let ext = (lct::EXT::Fdt as u32) << 24 | (version as u32) << 20 | fdt_id;
        self.data.extend(ext.to_be_bytes());
        lct::inc_hdr_len(&mut self.data, 1);
    }

    fn push_cenc(&mut self, cenc: u8) {
        /*
         0                   1                   2                   3
         0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |   HET = 193   |     CENC      |          Reserved             |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
          */
        let ext = (lct::EXT::Cenc as u32) << 24 | (cenc as u32) << 16;
        self.data.extend(ext.to_be_bytes());
        lct::inc_hdr_len(&mut self.data, 1);
    }

    fn push_sct(&mut self, time: u64) {
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
        let header: u32 = (lct::EXT::Time as u32) << 24 | (3u32 << 16) | (1u32 << 15) | (1u32 << 14);

        // Convert UTC to NTP
        let one_second_in_us = 1000000u64;
        let seconds_utc = time / one_second_in_us;
        let seconds_ntp = seconds_utc as u32 + 2208988800u32;
        let rest_ntp = (((time - (seconds_utc * one_second_in_us)) * (1u64 << 32)) / 1000000u64) as u32;

        self.data.extend(header.to_be_bytes());
        self.data.extend(seconds_ntp.to_be_bytes());
        self.data.extend(rest_ntp.to_be_bytes());
        lct::inc_hdr_len(&mut self.data, 3);
    }
}
