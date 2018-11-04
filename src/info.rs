extern crate mmal_sys as ffi;

use ffi::MMAL_STATUS_T;
use std::fmt;
use std::os::raw::c_char;
use std::ffi::CStr;
use std::mem;
use std::string::String;

use init::init;
use error::{CameraError, MmalError};

/// Contains information about attached cameras.
pub struct Info {
    pub cameras: Vec<CameraInfo>,
    // TODO: flashes?
}

impl fmt::Display for Info {
    /// Pretty prints a list of attached cameras.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Found {} camera(s)", self.cameras.len()).unwrap();

        // We can't iterate over all cameras because we will always have 4.
        // Alternatively, we could iterate and break early. Not sure if that is more rust-y
        self.cameras.iter().for_each(|camera| {
            write!(f, "\n  {}", camera).unwrap();
        });

        Ok(())
    }
}

/// Information about an attached camera. Created by the [`info`] function.
///
/// [`info`]: info()
#[derive(Clone, Debug)]
pub struct CameraInfo {
    pub port_id: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub lens_present: bool,
    pub camera_name: String,
}

impl fmt::Display for CameraInfo {
    /// Pretty prints this camera's name and its max resolution.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {}x{}",
            &self.camera_name, self.max_width, self.max_height
        )
    }
}

/// Retrieves info on attached cameras
pub fn info() -> Result<Info, CameraError> {
    init();

    unsafe {
        let info_type: *const c_char =
            ffi::MMAL_COMPONENT_DEFAULT_CAMERA_INFO.as_ptr() as *const c_char;
        let mut component: *mut ffi::MMAL_COMPONENT_T = mem::uninitialized(); // or ptr::null_mut()
        let status = ffi::mmal_component_create(info_type, &mut component);

        match status {
            MMAL_STATUS_T::MMAL_SUCCESS => {
                let mut info: ffi::MMAL_PARAMETER_CAMERA_INFO_T = mem::uninitialized();
                info.hdr.id = ffi::MMAL_PARAMETER_CAMERA_INFO as u32;
                info.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_INFO_T>() as u32;

                let status = ffi::mmal_port_parameter_get((*component).control, &mut info.hdr);

                match status {
                    MMAL_STATUS_T::MMAL_SUCCESS => {
                        let cameras = info.cameras
                            .iter()
                            .take(info.num_cameras as usize)
                            .map(|cam| CameraInfo {
                                port_id: cam.port_id,
                                max_width: cam.max_width,
                                max_height: cam.max_height,
                                lens_present: if cam.lens_present == 1 { true } else { false },
                                camera_name: CStr::from_ptr(cam.camera_name.as_ptr())
                                    .to_string_lossy()
                                    .into_owned(),
                            })
                            .collect();

                        ffi::mmal_component_destroy(component);

                        Ok(Info { cameras: cameras })
                    }
                    s => {
                        ffi::mmal_component_destroy(component);
                        Err(
                            MmalError::with_status("Failed to get camera info".to_owned(), s)
                                .into(),
                        )
                    }
                }
            }
            s => Err(
                MmalError::with_status("Failed to create camera component".to_owned(), s).into(),
            ),
        }
    }
}
