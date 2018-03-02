extern crate mmal_sys as ffi;

use std::os::raw::c_uint;

/// Settings for the camera.
///
/// ```
/// let settings = CameraSettings{
///     width: self.info.max_width,
///     height: self.info.max_height,
///     ..CameraSettings::default()
/// };
/// camera.set_camera_format(&settings)?;
/// ```
pub struct CameraSettings {
    pub encoding: c_uint,
    pub width: u32,  // 0 = max
    pub height: u32, // 0 = max
    pub zero_copy: bool,
    /// this will go away
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
