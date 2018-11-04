extern crate mmal_sys as ffi;

use std::os::raw::c_uint;

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
pub struct CameraSettings {
    pub encoding: c_uint,
    pub width: u32,  // 0 = max
    pub height: u32, // 0 = max
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
            zero_copy: false,
            use_encoder: true,
        }
    }
}
