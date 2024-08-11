
pub struct Service {

}

impl Service {
    pub fn new() -> Self {
        Self {

        }
    }
}


pub struct ServiceMgr {
    services: Vec<Service>
}

impl ServiceMgr {
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
        }
    }
}