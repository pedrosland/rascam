extern crate mmal_sys as ffi;
use std::error;
use ffi::MMAL_STATUS_T;
use std::fmt;

pub struct CameraError {
    message: &'static str,
    status_code: MMAL_STATUS_T::Type,
}

impl CameraError {
    // fn new(message: &'static str) -> CameraError {
    //     CameraError {
    //         message: message,
    //         status: MMAL_STATUS_T::MMAL_SUCCESS,
    //     }
    // }

    pub fn with_status(message: &'static str, status_code: MMAL_STATUS_T::Type) -> CameraError {
        CameraError {
            message: message,
            status_code: status_code,
        }
    }

    pub fn status(&self) -> &str {
        unsafe {
            ::std::ffi::CStr::from_ptr(ffi::mmal_status_to_string(self.status_code))
                .to_str()
                .unwrap()
        }
    }
}

#[test]
fn test_camera_error_status() {
    let mut err = CameraError {
        message: "testing",
        status_code: 0,
    };

    {
        let result = err.status();
        assert_eq!(result, "SUCCESS");
    }

    {
        err.status_code = 1;
        let result = err.status();
        assert_eq!(result, "ENOMEM");
    }

    {
        err.status_code = 3;
        let result = err.status();
        assert_eq!(result, "EINVAL");
    }

    // Note that there are other errors
}

impl fmt::Display for CameraError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.message)?;

        // Use 0 value (MMAL_STATUS) to indicate no status_code provided
        if self.status_code != MMAL_STATUS_T::MMAL_SUCCESS {
            let s = self.status();
            write!(f, " Status: {}", s)
        } else {
            Ok(())
        }
    }
}

impl error::Error for CameraError {
    fn description(&self) -> &str {
        // TODO: should we include the status here? If so, how? &str may make that hard.
        self.message
    }
}

impl fmt::Debug for CameraError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CameraError {{ message: {}, status: {}, status_code: {} }}",
            self.message,
            self.status(),
            self.status_code
        )
    }
}
