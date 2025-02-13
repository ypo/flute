use std::sync::{Arc, Mutex};

use rand::Rng;

use crate::common::lct;

use super::TOIMaxLength;

#[derive(Debug)]
struct ToiAllocatorInternal {
    toi_reserved: std::collections::HashSet<u128>,
    toi: u128,
    toi_max_length: TOIMaxLength,
}

#[derive(Debug)]
pub struct ToiAllocator {
    internal: Mutex<ToiAllocatorInternal>,
}

/// Struct containing a TOI
#[derive(Debug)]
pub struct Toi {
    allocator: Arc<ToiAllocator>,
    value: u128,
}

impl Drop for Toi {
    fn drop(&mut self) {
        self.allocator.release(self.value);
    }
}

impl Toi {
    /// Get Value of TOI
    pub fn get(&self) -> u128 {
        self.value
    }
}

impl ToiAllocatorInternal {
    fn new(toi_max_length: TOIMaxLength, toi_initial_value: Option<u128>) -> Self {
        let mut toi = match toi_initial_value {
            Some(0) => 1,
            Some(n) => n,
            None => {
                let mut rng = rand::rng();
                rng.random()
            }
        };

        toi = Self::to_max_length(toi, toi_max_length);
        if toi == lct::TOI_FDT {
            toi += 1;
        }

        Self {
            toi_reserved: std::collections::HashSet::new(),
            toi,
            toi_max_length,
        }
    }

    fn to_max_length(toi: u128, toi_max_length: TOIMaxLength) -> u128 {
        match toi_max_length {
            TOIMaxLength::ToiMax16 => toi & 0xFFFFu128,
            TOIMaxLength::ToiMax32 => toi & 0xFFFFFFFFu128,
            TOIMaxLength::ToiMax48 => toi & 0xFFFFFFFFFFFFu128,
            TOIMaxLength::ToiMax64 => toi & 0xFFFFFFFFFFFFFFFFu128,
            TOIMaxLength::ToiMax80 => toi & 0xFFFFFFFFFFFFFFFFFFFFu128,
            TOIMaxLength::ToiMax112 => toi,
        }
    }

    fn allocate(&mut self) -> u128 {
        let ret = self.toi;
        assert!(!self.toi_reserved.contains(&ret));
        self.toi_reserved.insert(ret);

        loop {
            self.toi = Self::to_max_length(self.toi + 1, self.toi_max_length);
            if self.toi == lct::TOI_FDT {
                self.toi = 1;
            }

            if !self.toi_reserved.contains(&self.toi) {
                break;
            }

            log::warn!("TOI {} is already used by a file or reserved", self.toi)
        }
        ret
    }

    fn release(&mut self, toi: u128) {
        let success = self.toi_reserved.remove(&toi);
        debug_assert!(success);
    }
}

impl ToiAllocator {
    pub fn new(toi_max_length: TOIMaxLength, toi_initial_value: Option<u128>) -> Arc<Self> {
        Arc::new(Self {
            internal: Mutex::new(ToiAllocatorInternal::new(toi_max_length, toi_initial_value)),
        })
    }

    pub fn allocate(allocator: &Arc<Self>) -> Box<Toi> {
        let mut db = allocator.internal.lock().unwrap();
        let toi = db.allocate();
        Box::new(Toi {
            allocator: allocator.clone(),
            value: toi,
        })
    }

    pub fn allocate_toi_fdt(allocator: &Arc<Self>) -> Box<Toi> {
        Box::new(Toi {
            allocator: allocator.clone(),
            value: 0,
        })
    }

    pub fn release(&self, toi: u128) {
        if toi == lct::TOI_FDT {
            return;
        }
        {
            let mut db = self.internal.lock().unwrap();
            db.release(toi);
        }
    }
}
