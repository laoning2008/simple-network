use std::collections::HashMap;
use std::sync::Mutex;
use crate::proto::{DeviceOfflineRequest, DeviceOnlineRequest};
use crate::service::{Service, ServiceMgr};

pub struct Device {
    id: String,
    name: String,
    address: String,
    service_mgr: ServiceMgr,
}

impl Device {
    pub fn new(id: &str, name: &str, address: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            address: address.to_string(),
            service_mgr: ServiceMgr::new(),
        }
    }


}

pub struct DeviceMgr {
    chan_to_device_map: Mutex<HashMap<u64, String>>,
    devices: Mutex<HashMap<String, Device>>,
}

impl DeviceMgr {
    pub fn new() -> Self {
        Self {
            chan_to_device_map: Default::default(),
            devices: Default::default(),
        }
    }

    pub async fn on_channel_closed(&self, channel_id: u64) {
        let chan_to_device_map = self.chan_to_device_map.lock().unwrap();
        if let Some(device_id) = chan_to_device_map.get(&channel_id) {
            let mut devices = self.devices.lock().unwrap();
            devices.remove(device_id);
        }
    }

    pub async fn on_device_online(&self, request: DeviceOnlineRequest) {
        let mut devices = self.devices.lock().unwrap();
        let device = request.device.unwrap();
        devices.remove(&device.device_id);
        devices.insert(device.device_id.clone(), Device::new(&device.device_id, &device.device_name, &device.device_address));
    }

    pub async fn on_device_offline(&self, request: DeviceOfflineRequest) {
        let mut devices = self.devices.lock().unwrap();
        devices.remove(&request.device_id);
    }

    // pub async fn get_online_devices(&self) -> Vec<Device> {
    //     let devices = self.devices.lock().unwrap();
    //     // for device in devices.values
    //     let mut all_devices: Vec<Device> = Vec::new();
    //     for (id, device) in devices.iter() {
    //         all_devices.push(device.c);
    //     }
    // }
}