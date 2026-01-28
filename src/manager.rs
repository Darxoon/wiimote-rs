use std::collections::HashMap;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::device::WiimoteDevice;
use crate::native::{wiimotes_scan, NativeWiimote};

type MutexWiimoteDevice = Arc<Mutex<WiimoteDevice>>;

/// Manages connections to Wii remotes.
/// Periodically checks for new connections of Wii remotes.
pub struct WiimoteManager {
    seen_devices: HashMap<String, MutexWiimoteDevice>,
    scan_interval: Duration,
    new_devices_receiver: Option<calloop::channel::Channel<MutexWiimoteDevice>>,
}

impl WiimoteManager {
    pub fn new() -> Arc<Mutex<Self>> {
        Self::new_with_interval(Duration::from_millis(500))
    }
    
    pub fn new_with_interval(scan_interval: Duration) -> Arc<Mutex<Self>> {
        // Make sure only one manager exists at a time
        static WIIMOTE_MANAGER_INITIALIZED: AtomicBool = AtomicBool::new(false);
        
        let prev_initialized = WIIMOTE_MANAGER_INITIALIZED.swap(true, Ordering::SeqCst);
        if prev_initialized {
            panic!("Several WiimoteManagers created in the same application!");
        }
        
        let (new_devices_sender, new_devices_receiver) = calloop::channel::channel();

        let manager = Arc::new(Mutex::new(Self {
            seen_devices: HashMap::new(),
            scan_interval,
            new_devices_receiver: Some(new_devices_receiver),
        }));

        let weak_manager = Arc::downgrade(&manager);
        std::thread::Builder::new()
            .name("wii-remote-scan".to_string())
            .spawn(move || {
                while let Some(manager) = weak_manager.upgrade() {
                    let interval = {
                        let mut manager = match manager.lock() {
                            Ok(m) => m,
                            Err(m) => m.into_inner(),
                        };

                        let new_devices = manager.scan();
                        let send_result = new_devices
                            .into_iter()
                            .try_for_each(|device| new_devices_sender.send(device));
                        if send_result.is_err() {
                            // Channel is disconnected, end scan thread
                            return;
                        }

                        manager.scan_interval
                    };

                    std::thread::sleep(interval);
                }
            })
            .expect("Failed to spawn Wii remote scan thread");

        manager
    }

    /// Set the interval at which the manager scans for Wii remotes.
    pub fn set_scan_interval(&mut self, scan_interval: Duration) {
        self.scan_interval = scan_interval;
    }

    /// Collection of Wii remotes that are connected or have been connected previously.
    #[must_use]
    pub fn seen_devices(&self) -> Vec<MutexWiimoteDevice> {
        self.seen_devices.values().map(Arc::clone).collect()
    }

    /// Receiver of newly connected Wii remotes.
    #[must_use]
    pub fn new_devices_receiver(&mut self) -> Option<calloop::channel::Channel<MutexWiimoteDevice>> {
        mem::take(&mut self.new_devices_receiver)
    }

    /// Scan for connected Wii remotes.
    fn scan(&mut self) -> Vec<MutexWiimoteDevice> {
        // Cleanup manually disconnected devices to send them to the receiver again.
        self.seen_devices.retain(|_, device| {
            device
                .try_lock()
                .map_or(true, |d| !d.manually_disconnected())
        });

        let mut native_devices = Vec::new();
        wiimotes_scan(&mut native_devices);

        let mut new_devices = Vec::new();

        for native_wiimote in native_devices {
            let identifier = native_wiimote.identifier();
            if let Some(existing_device) = self.seen_devices.get(&identifier) {
                let result = existing_device.lock().unwrap().reconnect(native_wiimote);
                if let Err(error) = result {
                    eprintln!("Failed to reconnect wiimote: {error:?}");
                }
            } else {
                match WiimoteDevice::new(native_wiimote) {
                    Ok(device) => {
                        let new_device = Arc::new(Mutex::new(device));
                        new_devices.push(Arc::clone(&new_device));
                        self.seen_devices.insert(identifier, new_device);
                    }
                    Err(error) => eprintln!("Failed to connect to wiimote: {error:?}"),
                }
            }
        }

        new_devices
    }
}
