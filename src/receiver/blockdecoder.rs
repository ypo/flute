use crate::common::{alc, oti};
use crate::error::FluteError;
use crate::fec;
use crate::fec::nocode;
use crate::fec::rscodec;
use crate::fec::FecDecoder;
use crate::tools::error::Result;

#[derive(Debug)]
pub struct BlockDecoder {
    pub completed: bool,
    pub initialized: bool,
    decoder: Option<Box<dyn FecDecoder>>,
}

impl BlockDecoder {
    pub fn new() -> BlockDecoder {
        BlockDecoder {
            completed: false,
            initialized: false,
            decoder: None,
        }
    }

    pub fn init(
        &mut self,
        oti: &oti::Oti,
        nb_source_symbols: u32,
        block_size: usize,
        sbn: u32,
    ) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        match oti.fec_encoding_id {
            oti::FECEncodingID::NoCode => {
                let codec = nocode::NoCodeDecoder::new(nb_source_symbols as usize);
                self.decoder = Some(Box::new(codec));
            }
            oti::FECEncodingID::ReedSolomonGF28 => {
                let codec = rscodec::RSGalois8Codec::new(
                    nb_source_symbols as usize,
                    oti.max_number_of_parity_symbols as usize,
                    oti.encoding_symbol_length as usize,
                )?;
                self.decoder = Some(Box::new(codec));
            }
            oti::FECEncodingID::ReedSolomonGF28UnderSpecified => {
                let codec = rscodec::RSGalois8Codec::new(
                    nb_source_symbols as usize,
                    oti.max_number_of_parity_symbols as usize,
                    oti.encoding_symbol_length as usize,
                )?;
                self.decoder = Some(Box::new(codec));
            }
            oti::FECEncodingID::ReedSolomonGF2M => {
                log::warn!("Not implemented")
            }
            oti::FECEncodingID::RaptorQ => {
                if oti.raptorq_scheme_specific.is_none() {
                    return Err(FluteError::new("RaptorQ Scheme not found"));
                }

                let codec = fec::raptorq::RaptorQDecoder::new(
                    sbn,
                    nb_source_symbols as usize,
                    oti.encoding_symbol_length as usize,
                    oti.raptorq_scheme_specific.as_ref().unwrap(),
                );
                self.decoder = Some(Box::new(codec));
            }
            oti::FECEncodingID::Raptor => {
                if oti.raptor_scheme_specific.is_none() {
                    return Err(FluteError::new("Raptor Scheme not found"));
                }

                let codec = fec::raptor::RaptorDecoder::new(nb_source_symbols as usize, block_size);
                self.decoder = Some(Box::new(codec));
            }
        }

        self.initialized = true;
        Ok(())
    }

    pub fn source_block(&self) -> Result<&[u8]> {
        if self.decoder.is_none() {
            return Err(FluteError::new("Fail to decode block"));
        }

        self.decoder.as_ref().unwrap().source_block()
    }

    pub fn deallocate(&mut self) {
        self.decoder = None;
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt, payload_id: &alc::PayloadID) {
        assert!(self.initialized);

        if self.completed {
            return;
        }

        let payload = &pkt.data[pkt.data_payload_offset..];
        let decoder = self.decoder.as_mut().unwrap();
        decoder.push_symbol(payload, payload_id.esi);

        if decoder.can_decode() {
            self.completed = decoder.decode();
            if self.completed {
                log::debug!("Block completed");
            }
        }
    }
}
