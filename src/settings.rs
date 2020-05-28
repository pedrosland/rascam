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
///
/// Note that not all options are available on all RaspberryPi Models.
/// As an example, the Pi4 doesn't support H263, MPEG2, MPEG4 or VC1 encodings.
/// See https://github.com/raspberrypi/documentation/blob/master/raspbian/applications/vcgencmd.md#codec_enabled-type
/// and https://www.raspberrypi.org/forums/viewtopic.php?t=250419
#[derive(Clone, Debug)]
pub struct CameraSettings {
    // shared
    pub encoding: c_uint,
    pub width: u32,  // 0 = max
    pub height: u32, // 0 = max
    pub zero_copy: bool,
    // image
    pub iso: ISO,
    /// `use_encoder` will go away
    pub use_encoder: bool,
    // video
    pub framerate: u32,
    pub video_profile: ffi::MMAL_VIDEO_PROFILE_T,
    pub video_level: ffi::MMAL_VIDEO_LEVEL_T,
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
            framerate: 30,
            video_profile: ffi::MMAL_VIDEO_PROFILE_H264_HIGH,
            video_level: ffi::MMAL_VIDEO_LEVEL_H264_4,
        }
    }
}

impl CameraSettings {
    pub(crate) fn is_video(&self) -> bool {
        match self.encoding {
            ffi::MMAL_ENCODING_H264
            | ffi::MMAL_ENCODING_VP8
            | ffi::MMAL_ENCODING_MP4V
            | ffi::MMAL_ENCODING_MP2V
            | ffi::MMAL_ENCODING_WVC1
            | ffi::MMAL_ENCODING_H263 => true,
            _ => false,
        }
    }
}
