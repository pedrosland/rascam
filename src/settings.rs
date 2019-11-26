use mmal_sys as ffi;

use std::os::raw::c_uint;

pub type ISO = u32;

pub const ISO_AUTO: ISO = 0;
pub const ISO_100: ISO = 100;
pub const ISO_125: ISO = 125;
pub const ISO_160: ISO = 160;
pub const ISO_200: ISO = 200;
pub const ISO_250: ISO = 250;
pub const ISO_320: ISO = 320;
pub const ISO_400: ISO = 400;
pub const ISO_500: ISO = 500;
pub const ISO_640: ISO = 640;
pub const ISO_800: ISO = 800;
pub const ISO_1000: ISO = 1000;
pub const ISO_1250: ISO = 1250;
pub const ISO_1600: ISO = 1600;
pub const ISO_2000: ISO = 2000;
pub const ISO_2500: ISO = 2500;
pub const ISO_3200: ISO = 3200;

/// Settings for the camera.
///
/// ```
/// # use rascam::{CameraError, CameraSettings, SimpleCamera};
/// #
/// # let info = rascam::info().unwrap().cameras[0].clone();
/// # let mut camera = SimpleCamera::new(info.clone()).unwrap();
/// #
/// let settings = CameraSettings{
///     width: info.max_width,
///     height: info.max_height,
///     ..CameraSettings::default()
/// };
/// camera.configure(settings);
/// ```
#[derive(Debug)]
pub struct CameraSettings {
    pub encoding: c_uint,
    pub width: u32,  // 0 = max
    pub height: u32, // 0 = max
    pub iso: ISO,
    pub zero_copy: bool,
    /// `use_encoder` will go away
    pub use_encoder: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        CameraSettings {
            encoding: ffi::MMAL_ENCODING_JPEG,
            width: 0,
            height: 0,
            iso: ISO_AUTO,
            zero_copy: false,
            use_encoder: true,
        }
    }
}
