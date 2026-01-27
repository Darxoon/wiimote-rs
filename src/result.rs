use std::fmt::Display;

#[derive(Debug)]
pub enum WiimoteError {
    WiimoteDeviceError(WiimoteDeviceError),
    Disconnected,
}

impl Display for WiimoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WiimoteError::WiimoteDeviceError(wiimote_device_error) => wiimote_device_error.fmt(f),
            WiimoteError::Disconnected => write!(f, "Disconnected"),
        }
    }
}

impl std::error::Error for WiimoteError {}

#[derive(Debug)]
pub enum WiimoteDeviceError {
    InvalidVendorID(u16),
    InvalidProductID(u16),
    MissingData,
    InvalidChecksum,
    InvalidData,
}

impl From<WiimoteDeviceError> for WiimoteError {
    fn from(e: WiimoteDeviceError) -> Self {
        Self::WiimoteDeviceError(e)
    }
}

impl Display for WiimoteDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WiimoteDeviceError::InvalidVendorID(id) => write!(f, "Invalid vendor ID {id}"),
            WiimoteDeviceError::InvalidProductID(id) => write!(f, "Invalid product ID {id}"),
            WiimoteDeviceError::MissingData => write!(f, "Missing data"),
            WiimoteDeviceError::InvalidChecksum => write!(f, "Invalid checksum"),
            WiimoteDeviceError::InvalidData => write!(f, "Invalid data"),
        }
    }
}

impl std::error::Error for WiimoteDeviceError {}

pub type WiimoteResult<T> = Result<T, WiimoteError>;
