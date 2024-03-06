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
    pub fn to_i32(&self) -> i32 {
        *self as i32
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
    pub fn to_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy)]
/// Auto White Balance Mode
// no sense supporting Off if we don't also support awb_gains_r & awb_gains_b
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
    pub fn to_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy)]
/// Flicker reduction mode
// values from MMAL_PARAM_FLICKERAVOID_T in https://github.com/raspberrypi/userland/blob/master/interface/mmal/mmal_parameters_camera.h
pub enum FlickerAvoidMode {
    Off = 0,
    Auto = 1,
    Avoid50Hz = 2,
    Avoid60Hz = 3,
}

impl FlickerAvoidMode {
    pub fn to_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy)]
/// Image rotation
pub enum Rotation {
    Rotate0 = 0,
    Rotate90 = 90,
    Rotate180 = 180,
    Rotate270 = 270,
}

impl Rotation {
    pub fn to_i32(&self) -> i32 {
        *self as i32
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
    // shutter_speed: 0 = auto, otherwise the shutter speed in microseconds
    pub shutter_speed: u32,
    /// Exposure mode
    pub exposure_mode: ExposureMode,
    /// Meterng Mode
    pub metering_mode: MeteringMode,
    /// White Balance
    pub awb_mode: AwbMode,
    /// EV compensation in steps of 1/6 stop (-25 to +25)
    pub exposure_compensation: i32,
    /// Brightness 0% to 100%, default = 50%
    pub brightness: u32,
    /// Contrast -100% to +100%, default = 0%
    pub contrast: i32,
    // Saturation  -100% to 100%, default = 0%
    pub saturation: i32,
    // Sharpness  -100% to 100%, default = 0%
    pub sharpness: i32,
    // rotation (0, 90, 180, or 270), default = 0
    pub rotation: Rotation,
    // H&V flip , default = false
    pub horizontal_flip: bool,
    pub vertical_flip: bool,

    // flicker avoidance mode  (Off, Auto, 50Hz, 60Hz), default = Auto
    pub flicker_avoid: FlickerAvoidMode,
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
            shutter_speed: 0,
            exposure_mode: ExposureMode::Auto,
            metering_mode: MeteringMode::Average,
            awb_mode: AwbMode::Auto,
            exposure_compensation: 0,
            brightness: 50,
            contrast: 0,
            saturation: 0,
            sharpness: 0,
            rotation: Rotation::Rotate0,
            horizontal_flip: false,
            vertical_flip: false,
            flicker_avoid: FlickerAvoidMode::Auto,
            zero_copy: false,
            use_encoder: true,
        }
    }
}