use crate::common::udpendpoint::UDPEndpoint;

struct TSI {
    endpoints: std::collections::HashMap<UDPEndpoint, u64>,
}

pub struct TSIFilter {
    tsi: std::collections::HashMap<u64, TSI>,
    endpoint_bypass: std::collections::HashMap<UDPEndpoint, u64>,
}

impl std::fmt::Debug for TSIFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FecEncoder {{  }}")
    }
}

impl TSIFilter {
    pub fn new() -> Self {
        TSIFilter {
            tsi: std::collections::HashMap::new(),
            endpoint_bypass: std::collections::HashMap::new(),
        }
    }

    pub fn add_endpoint_bypass(&mut self, endpoint: UDPEndpoint) {
        match self.endpoint_bypass.get_mut(&endpoint) {
            Some(bypass) => *bypass += 1,
            None => {
                self.endpoint_bypass.insert(endpoint, 1);
            }
        }
    }

    pub fn remove_endpoint_bypass(&mut self, endpoint: &UDPEndpoint) {
        if let Some(bypass) = self.endpoint_bypass.get_mut(endpoint) {
            if *bypass > 1 {
                *bypass -= 1
            } else {
                self.endpoint_bypass.remove(endpoint);
            }
        }
    }

    pub fn add(&mut self, endpoint: UDPEndpoint, tsi: u64) {
        match self.tsi.get_mut(&tsi) {
            Some(tsi) => tsi.add(endpoint),
            None => {
                self.tsi.insert(tsi, TSI::new(endpoint));
            }
        }
    }

    pub fn remove(&mut self, endpoint: &UDPEndpoint, tsi: u64) {
        if let Some(t) = self.tsi.get_mut(&tsi) {
            t.remove(endpoint);
            if t.is_empty() {
                self.tsi.remove(&tsi);
            }
        }
    }

    pub fn is_valid(&self, endpoint: &UDPEndpoint, tsi: u64) -> bool {
        if self.endpoint_bypass.contains_key(endpoint) {
            return true;
        }

        if let Some(t) = self.tsi.get(&tsi) {
            return t.is_valid(endpoint);
        }

        false
    }
}

impl TSI {
    fn new(endpoint: UDPEndpoint) -> Self {
        let mut output = Self {
            endpoints: std::collections::HashMap::new(),
        };
        output.endpoints.insert(endpoint, 1);
        output
    }

    fn add(&mut self, endpoint: UDPEndpoint) {
        match self.endpoints.get_mut(&endpoint) {
            Some(a) => *a += 1,
            None => {
                self.endpoints.insert(endpoint.clone(), 1);
            }
        }
    }

    fn remove(&mut self, endpoint: &UDPEndpoint) {
        if let Some(v) = self.endpoints.get_mut(endpoint) {
            if *v > 1 {
                *v -= 1;
            } else {
                self.endpoints.remove(endpoint);
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }

    fn is_valid(&self, endpoint: &UDPEndpoint) -> bool {
        if self.endpoints.contains_key(endpoint) {
            return true;
        }

        let mut endpoint_no_src = endpoint.clone();
        endpoint_no_src.source_address = None;
        self.endpoints.contains_key(&endpoint_no_src)
    }
}
