use ffi::MMAL_STATUS_T;
use mmal_sys as ffi;
use std::convert::From;
use std::error;
use std::fmt;
use std::io;
use std::sync::mpsc;

/// Represents an error from the MMAL library.
pub struct MmalError {
    message: String,
    status_code: MMAL_STATUS_T::Type,
}

impl MmalError {
    pub fn with_status(message: String, status_code: MMAL_STATUS_T::Type) -> MmalError {
        MmalError {
            message,
            status_code,
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
    let mut err = MmalError {
        message: "testing".to_string(),
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

impl fmt::Display for MmalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.message)?;

        // Use 0 value (MMAL_STATUS) to indicate no status_code provided
        if self.status_code != MMAL_STATUS_T::MMAL_SUCCESS {
            let s = self.status();
            write!(f, " Status: {}", s)
        } else {
            Ok(())
        }
    }
}

impl error::Error for MmalError {
    fn description(&self) -> &str {
        // TODO: should we include the status here? If so, how? &str may make that hard.
        &self.message
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl fmt::Debug for MmalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MmalError {{ message: {}, status: {}, status_code: {} }}",
            self.message,
            self.status(),
            self.status_code,
        )
    }
}

/// Represents any error returned when calling a camera function.
#[derive(Debug)]
pub struct CameraError(Box<ErrorKind>);

impl CameraError {
    /// Return the specific type of this error.
    pub fn kind(&self) -> &ErrorKind {
        &self.0
    }

    /// Unwrap this error into its underlying type.
    pub fn into_kind(self) -> ErrorKind {
        *self.0
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    Mmal(MmalError),
    Recv(mpsc::RecvError),
    Io(io::Error),

    /// Hints that destructuring should not be exhaustive.
    ///
    /// This enum may grow additional variants, so this makes sure clients
    /// don't count on exhaustive matching. (Otherwise, adding a new variant
    /// could break existing code.)
    #[doc(hidden)]
    __Nonexhaustive,
}

impl fmt::Display for CameraError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *(self.kind()) {
            ErrorKind::Mmal(ref err) => write!(f, "MMAL error: {}", err),
            ErrorKind::Recv(ref err) => write!(f, "Recv error: {}", err),
            ErrorKind::Io(ref err) => write!(f, "IO error: {}", err),
            _ => unreachable!(),
        }
    }
}

impl error::Error for CameraError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *(self.kind()) {
            ErrorKind::Mmal(ref err) => Some(err),
            ErrorKind::Recv(ref err) => Some(err),
            ErrorKind::Io(ref err) => Some(err),
            _ => unreachable!(),
        }
    }
}

impl From<MmalError> for ErrorKind {
    fn from(err: MmalError) -> ErrorKind {
        ErrorKind::Mmal(err)
    }
}

impl From<MmalError> for CameraError {
    fn from(err: MmalError) -> CameraError {
        CameraError(Box::new(ErrorKind::Mmal(err)))
    }
}

impl From<mpsc::RecvError> for CameraError {
    fn from(err: mpsc::RecvError) -> CameraError {
        CameraError(Box::new(ErrorKind::Recv(err)))
    }
}

impl From<io::Error> for CameraError {
    fn from(err: io::Error) -> CameraError {
        CameraError(Box::new(ErrorKind::Io(err)))
    }
}
