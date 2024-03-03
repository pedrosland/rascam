use mmal_sys as ffi;

use std::os::raw::c_uint;

#[derive(Debug, Clone, Copy)]
pub enum ISO {
    IsoAuto = 0,
    Iso125 = 125,
    Iso160 = 160,
    Iso200 = 200,
    Iso250 = 250,
    Iso320 = 320,
    Iso400 = 400,
    Iso500 = 500,
    Iso640 = 640,
    Iso800 = 800,
    Iso1000 = 1000,
    Iso1250 = 1250,
    Iso1600 = 1600,
    Iso2000 = 2000,
    Iso2500 = 2500,
    Iso3200 = 3200,
}

impl ISO {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MeteringMode {
    // values from MMAL_PARAM_EXPOSUREMETERINGMODE_T in https://github.com/raspberrypi/userland/blob/master/interface/mmal/mmal_parameters_camera.h
    Average = 0,
    Spot = 1,
    Backlit = 2,
    Matrix = 3,
}

impl MeteringMode {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }
}

#[derive(Debug, Clone, Copy)]
// values from MMAL_PARAM_EXPOSUREMODE_T in https://github.com/raspberrypi/userland/blob/master/interface/mmal/mmal_parameters_camera.h
pub enum ExposureMode {
    Off = 0,
    Auto = 1,
    Night = 2,
    NightPreview = 3,
    Backlight = 4,
    Spotlight = 5,
    Sports = 6,
    Snow = 7,
    Beach = 8,
    VeryLong = 9,
}

impl ExposureMode {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }
}

#[derive(Debug, Clone, Copy)]
/// Auto White Balance Mode
// no sense supporting Off is we don't also support awb_gains_r & awb_gains_b
// values from MMAL_PARAM_AWBMODE_T in https://github.com/raspberrypi/userland/blob/master/interface/mmal/mmal_parameters_camera.h
pub enum AwbMode {
    Auto = 1,
    Sunlight = 2,
    Cloud = 3,
    Shade = 4,
    Tungsten = 5,
    Fluorescent = 6,
    Incandescent = 7,
}

impl AwbMode {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }
}

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
#[derive(Debug, Clone, Copy)]
pub struct CameraSettings {
    pub encoding: c_uint,
    /// image width in pixels, 0 = maximum
    pub width: u32,
    /// image height in pixels, 0 = maximum
    pub height: u32,
    /// ISO. Default is Auto
    pub iso: ISO,
    /// Exposure mode
    pub exposure_mode: ExposureMode,
    /// Meteriing Mode
    pub metering_mode: MeteringMode,
    /// White Balance
    pub awb_mode: AwbMode,
    /// EV compensation in steps of 1/6 stop (-25 to +25)
    pub exposure_compensation: i32,
    /// Brightness 0% to 100%, default = 50%
    pub brightness: u32,
    /// Contrast -100% to +100%, default = 0%
    pub contrast: i32,
    // TODO: add saturation -100% to 100%
    // TODO: add sharpness  -100% to 100%
    // TODO: add rotation (0, 90, 180, or 270)
    // TODO: add H&V flip
    // TODO: Do we need shutter speed? - probably not
    // TODO: Do we need flicker avoidance mode  (Off, 50Hz, 60Hz)?
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
            iso: ISO::IsoAuto,
            exposure_mode: ExposureMode::Auto,
            metering_mode: MeteringMode::Average,
            awb_mode: AwbMode::Auto,
            exposure_compensation: 0,
            brightness: 50,
            contrast: 0,
            zero_copy: false,
            use_encoder: true,
        }
    }
}
