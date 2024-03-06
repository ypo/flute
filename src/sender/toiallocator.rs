use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct ToiAllocator {
    toi_reserved: Mutex<std::collections::HashSet<u128>>,
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

impl ToiAllocator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            toi_reserved: Mutex::new(std::collections::HashSet::new()),
        })
    }

    pub fn allocate(allocator: &Arc<Self>, toi: u128) -> Arc<Toi> {
        {
            let mut db = allocator.toi_reserved.lock().unwrap();
            let success = db.insert(toi);
            debug_assert!(success);
        }

        Arc::new(Toi {
            allocator: allocator.clone(),
            value: toi,
        })
    }

    pub fn release(&self, toi: u128) {
        let mut db = self.toi_reserved.lock().unwrap();
        let success = db.remove(&toi);
        debug_assert!(success);
    }

    pub fn contains(&self, toi: &u128) -> bool {
        let db = self.toi_reserved.lock().unwrap();
        db.contains(toi)
    }
}
