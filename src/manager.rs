use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_utils::atomic::AtomicCell;
use futures_channel::mpsc::{Receiver, channel};

use crate::device::WiimoteDevice;
use crate::native::{wiimotes_scan, NativeWiimote};

type MutexWiimoteDevice = Arc<Mutex<WiimoteDevice>>;

/// Manages connections to Wii remotes.
/// Periodically checks for new connections of Wii remotes.
pub struct WiimoteManager {
    pub new_devices_receiver: Option<Receiver<MutexWiimoteDevice>>,
    
    scan_interval: Arc<AtomicCell<Duration>>,
}

impl WiimoteManager {
    pub fn new() -> Self {
        Self::new_with_interval(Duration::from_millis(500))
    }
    
    pub fn new_with_interval(scan_interval: Duration) -> Self {
        // Make sure only one manager exists at a time
        static WIIMOTE_MANAGER_INITIALIZED: AtomicBool = AtomicBool::new(false);
        
        let prev_initialized = WIIMOTE_MANAGER_INITIALIZED.swap(true, Ordering::SeqCst);
        if prev_initialized {
            panic!("Several WiimoteManagers created in the same application!");
        }
        
        let (mut new_devices_sender, new_devices_receiver) = channel::<MutexWiimoteDevice>(8);
        
        let scan_interval = Arc::new(AtomicCell::new(scan_interval));
        let weak_scan_interval = Arc::downgrade(&scan_interval);

        std::thread::Builder::new()
            .name("wii-remote-scan".to_string())
            .spawn(move || {
                let mut seen_devices = HashMap::new();
                let mut new_devices: Vec<MutexWiimoteDevice> = Vec::new();
                let mut new_devices_queue: Vec<MutexWiimoteDevice> = Vec::new();
                
                while let Some(scan_interval) = weak_scan_interval.upgrade() {
                    Self::scan(&mut new_devices, &mut seen_devices);
                    
                    for device in new_devices.drain(..) {
                        if let Err(err) = new_devices_sender.try_send(device) {
                            if err.is_full() {
                                new_devices_queue.push(err.into_inner());
                            } else {
                                // Disconnected
                                return;
                            }
                        }
                    }
                    
                    new_devices.extend(new_devices_queue.drain(..));

                    std::thread::sleep(scan_interval.load());
                }
            })
            .expect("Failed to spawn Wii remote scan thread");

        Self {
            new_devices_receiver: Some(new_devices_receiver),
            scan_interval,
        }
    }

    /// Set the interval at which the manager scans for Wii remotes.
    pub fn set_scan_interval(&mut self, scan_interval: Duration) {
        self.scan_interval.store(scan_interval);
    }

    /// Scan for connected Wii remotes.
    fn scan(new_devices: &mut Vec<MutexWiimoteDevice>, seen_devices: &mut HashMap<String, MutexWiimoteDevice>) {
        // Cleanup manually disconnected devices to send them to the receiver again.
        seen_devices.retain(|_, device| {
            device
                .try_lock()
                .map_or(true, |d| !d.manually_disconnected())
        });

        let mut native_devices = Vec::new();
        wiimotes_scan(&mut native_devices);

        for native_wiimote in native_devices {
            let identifier = native_wiimote.identifier();
            if let Some(existing_device) = seen_devices.get(&identifier) {
                let result = existing_device.lock().unwrap().reconnect(native_wiimote);
                if let Err(error) = result {
                    eprintln!("Failed to reconnect wiimote: {error:?}");
                }
            } else {
                match WiimoteDevice::new(native_wiimote) {
                    Ok(device) => {
                        let new_device = Arc::new(Mutex::new(device));
                        new_devices.push(Arc::clone(&new_device));
                        seen_devices.insert(identifier, new_device);
                    }
                    Err(error) => eprintln!("Failed to connect to wiimote: {error:?}"),
                }
            }
        }
    }
}
