//! Access the native Raspberry Pi camera.
//!
//! This module uses the MMAL library ([mmal-sys]) to access the Raspberry Pi's camera
//! in a friendly way.
//!
//! [mmal-sys]: https://crates.io/crates/mmal-sys

#![feature(ptr_internals)]
extern crate libc;
extern crate mmal_sys as ffi;
extern crate parking_lot;
// extern crate futures;
use ffi::MMAL_STATUS_T;
use std::fmt;
use std::os::raw::c_char;
use std::ffi::CStr;
use std::mem;
use std::ptr::NonNull;
use std::slice;
use std::string::String;
use std::sync::{Arc, Once, ONCE_INIT};
use std::sync::mpsc;
use std::ptr;
use parking_lot::Mutex;

mod error;
mod settings;

pub use error::{CameraError, MmalError};
pub use settings::CameraSettings;

const MMAL_CAMERA_PREVIEW_PORT: isize = 0;
const MMAL_CAMERA_VIDEO_PORT: isize = 1;
const MMAL_CAMERA_CAPTURE_PORT: isize = 2;

/// Video render needs at least 2 buffers.
const VIDEO_OUTPUT_BUFFERS_NUM: u32 = 3;

const PREVIEW_FRAME_RATE_NUM: i32 = 0;
const PREVIEW_FRAME_RATE_DEN: i32 = 1;

// TODO: what about the rest of these formats?
pub use ffi::MMAL_ENCODING_JPEG;
pub use ffi::MMAL_ENCODING_GIF;
pub use ffi::MMAL_ENCODING_PNG;

pub use ffi::MMAL_ENCODING_OPAQUE;

pub use ffi::MMAL_ENCODING_RGB24;

// type Future2 = Box<Future<Item = [u8], Error = ffi::MMAL_STATUS_T::Type>>;

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

/// This function must be called before any mmal work. Failure to do so will cause errors like:
///
/// mmal: mmal_component_create_core: could not find component 'vc.camera_info'
///
/// See this for more info https://github.com/thaytan/gst-rpicamsrc/issues/28
fn init() {
    static INIT: Once = ONCE_INIT;
    INIT.call_once(|| unsafe {
        ffi::bcm_host_init();
        ffi::vcos_init();
        ffi::mmal_vc_init();
    });
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

struct Userdata {
    pool: NonNull<ffi::MMAL_POOL_T>,
    _guard: Arc<Mutex<()>>,
    sender: mpsc::SyncSender<Option<BufferGuard>>,
}

/// Guard around a buffer header.
///
/// Releases buffer header when it is dropped.
#[derive(Debug)]
pub struct BufferGuard {
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
    pool: NonNull<ffi::MMAL_POOL_T>,
    complete: bool,
}

impl BufferGuard {
    pub fn new(
        port: *mut ffi::MMAL_PORT_T,
        buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
        pool: NonNull<ffi::MMAL_POOL_T>,
        complete: bool,
    ) -> BufferGuard {
        BufferGuard {
            port: port,
            buffer: buffer,
            pool: pool,
            complete: complete,
        }
    }

    /// Indicates if an image has been captured and this is the end of the image.
    pub fn is_complete(&self) -> bool {
        return self.complete;
    }

    /// Creates a slice representing the raw bytes of the image.
    ///
    /// The data buffer is owned by the camera and must be copied to keep it around after the
    /// BufferGuard is dropped.
    pub fn get_bytes(&self) -> &[u8] {
        unsafe {
            let buffer = *self.buffer;
            slice::from_raw_parts(
                buffer.data.offset(buffer.offset as isize),
                buffer.length as usize,
            )
        }
    }
}

impl Drop for BufferGuard {
    /// Unlocks and releases the buffer header. Gets new buffer from pool and passes it to
    /// the camera.
    fn drop(&mut self) {
        unsafe {
            ffi::mmal_buffer_header_mem_unlock(self.buffer);

            // Release buffer back to the pool
            ffi::mmal_buffer_header_release(self.buffer);

            // Get new buffer from the pool and send it to the port (if still open)
            if (*self.port).is_enabled > 0 {
                let mut status = ffi::MMAL_STATUS_T::MMAL_STATUS_MAX;
                let new_buffer: *mut ffi::MMAL_BUFFER_HEADER_T =
                    ffi::mmal_queue_get(self.pool.as_ref().queue);

                if !new_buffer.is_null() {
                    status = ffi::mmal_port_send_buffer(self.port, new_buffer);
                }

                if new_buffer.is_null() || status != MMAL_STATUS_T::MMAL_SUCCESS {
                    println!("Unable to return the buffer to the port");
                }
            }

            if self.complete {
                if (*self.port).userdata != ptr::null_mut() {
                    drop_port_userdata(self.port);
                }
                println!("complete");
            }
        }
    }
}

#[repr(C)]
pub struct SeriousCamera {
    camera: NonNull<ffi::MMAL_COMPONENT_T>,
    enabled: bool,
    camera_port_enabled: bool,
    still_port_enabled: bool,
    pool: Option<NonNull<ffi::MMAL_POOL_T>>,
    mutex: Arc<Mutex<()>>,

    encoder: Option<NonNull<ffi::MMAL_COMPONENT_T>>,
    encoder_created: bool,
    encoder_enabled: bool,
    encoder_control_port_enabled: bool,
    encoder_output_port_enabled: bool,

    connection: Option<NonNull<ffi::MMAL_CONNECTION_T>>,
    connection_created: bool,

    preview: Option<NonNull<ffi::MMAL_COMPONENT_T>>,
    preview_created: bool,

    use_encoder: bool,
}

impl SeriousCamera {
    pub fn new() -> Result<SeriousCamera, CameraError> {
        init();
        unsafe {
            let mut camera_ptr: *mut ffi::MMAL_COMPONENT_T = mem::uninitialized();
            let component: *const c_char =
                ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const c_char;
            let status = ffi::mmal_component_create(component, &mut camera_ptr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(SeriousCamera {
                    camera: NonNull::new(camera_ptr).unwrap(),
                    enabled: false,
                    camera_port_enabled: false,
                    pool: None,
                    mutex: Arc::new(Mutex::new(())),
                    still_port_enabled: false,
                    // this is really a hack. ideally these objects wouldn't be structured this way
                    encoder_created: false,
                    encoder_enabled: false,
                    encoder_control_port_enabled: false,
                    encoder_output_port_enabled: false,
                    encoder: None,
                    connection_created: false,
                    connection: None,
                    preview_created: false,
                    preview: None,
                    use_encoder: false,
                }),
                s => Err(MmalError::with_status("Could not create camera".to_owned(), s).into()),
            }
        }
    }

    pub fn set_camera_num(&mut self, num: u8) -> Result<(), CameraError> {
        unsafe {
            let mut param: ffi::MMAL_PARAMETER_INT32_T = mem::uninitialized();
            param.hdr.id = ffi::MMAL_PARAMETER_CAMERA_NUM as u32;
            param.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_INT32_T>() as u32;
            param.value = num as i32;

            let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut param.hdr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => {
                    Err(MmalError::with_status("Unable to set camera number".to_owned(), s).into())
                }
            }
        }
    }

    pub fn create_encoder(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut encoder_ptr: *mut ffi::MMAL_COMPONENT_T = mem::uninitialized();
            let component: *const c_char =
                ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const c_char;
            let status = ffi::mmal_component_create(component, &mut encoder_ptr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.encoder = Some(NonNull::new(encoder_ptr).unwrap());
                    self.encoder_created = true;
                    Ok(())
                }
                s => Err(MmalError::with_status("Unable to create encoder".to_owned(), s).into()),
            }
        }
    }

    pub fn connect_encoder(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut connection_ptr: *mut ffi::MMAL_CONNECTION_T = mem::uninitialized();
            let status = ffi::mmal_connection_create(
                &mut connection_ptr,
                *self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT),
                *self.encoder.unwrap().as_ref().input.offset(0),
                ffi::MMAL_CONNECTION_FLAG_TUNNELLING
                    | ffi::MMAL_CONNECTION_FLAG_ALLOCATION_ON_INPUT,
            );
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to create camera->encoder connection".to_owned(),
                    status,
                ).into());
            }

            self.connection = Some(NonNull::new(connection_ptr).unwrap());
            self.connection_created = true;
            let status = ffi::mmal_connection_enable(&mut *connection_ptr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => Err(MmalError::with_status(
                    "Unable to enable camera->encoder connection".to_owned(),
                    s,
                ).into()),
            }
            // Ok(())
        }
    }

    pub fn enable_control_port(&mut self, get_buffers: bool) -> Result<(), CameraError> {
        unsafe {
            let cb: ffi::MMAL_PORT_BH_CB_T = if get_buffers {
                Some(camera_buffer_callback)
            } else {
                Some(camera_control_callback)
            };
            let status = ffi::mmal_port_enable(self.camera.as_ref().control, cb);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.camera_port_enabled = true;
                    Ok(())
                }
                s => Err(
                    MmalError::with_status("Unable to enable control port".to_owned(), s).into(),
                ),
            }
        }
    }

    pub fn enable_encoder_port(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_port_enable(
                *self.encoder.unwrap().as_ref().output.offset(0),
                Some(camera_buffer_callback),
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.encoder_output_port_enabled = true;
                    Ok(())
                }
                s => Err(
                    MmalError::with_status("Unable to enable encoder port".to_owned(), s).into(),
                ),
            }
        }
    }

    pub unsafe fn set_buffer_callback(
        &mut self,
        sender: mpsc::SyncSender<Option<BufferGuard>>,
    ) {
        let port = if self.use_encoder {
            (*self.encoder.unwrap().as_ref().output.offset(0))
        } else {
            (*self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT))
        };

        let userdata = Userdata {
            pool: self.pool.unwrap(),
            sender: sender,
            _guard: Arc::clone(&self.mutex),
        };

        if (*port).userdata != ptr::null_mut() {
            panic!("port.userdata was not null");
        }

        (*port).userdata = Box::into_raw(Box::new(userdata)) as *mut ffi::MMAL_PORT_USERDATA_T;
    }

    pub fn enable_still_port(&mut self) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let status = ffi::mmal_port_enable(
                *self.camera.as_ref().output.offset(2),
                Some(camera_buffer_callback),
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.still_port_enabled = true;
                    Ok(1)
                }
                e => Err(e),
            }
        }
    }

    pub fn set_camera_params(&mut self, info: &CameraInfo) -> Result<(), CameraError> {
        unsafe {
            let mut cfg: ffi::MMAL_PARAMETER_CAMERA_CONFIG_T = mem::uninitialized();
            cfg.hdr.id = ffi::MMAL_PARAMETER_CAMERA_CONFIG as u32;
            cfg.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_CONFIG_T>() as u32;

            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L706
            cfg.max_stills_w = info.max_width;
            cfg.max_stills_h = info.max_height;
            cfg.stills_yuv422 = 0;
            cfg.one_shot_stills = 1;
            cfg.max_preview_video_w = info.max_width;
            cfg.max_preview_video_h = info.max_height;
            cfg.num_preview_video_frames = 1;
            cfg.stills_capture_circular_buffer_height = 0;
            cfg.fast_preview_resume = 0;
            cfg.use_stc_timestamp = ffi::MMAL_PARAMETER_CAMERA_CONFIG_TIMESTAMP_MODE_T::MMAL_PARAM_TIMESTAMP_MODE_RESET_STC;

            let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut cfg.hdr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => Err(MmalError::with_status(
                    "Unable to set control port parmaeter".to_owned(),
                    s,
                ).into()),
            }
        }
    }

    pub fn set_camera_format(&mut self, settings: &CameraSettings) -> Result<(), CameraError> {
        unsafe {
            self.use_encoder = settings.use_encoder;
            let mut encoding = settings.encoding;

            let output = self.camera.as_ref().output;
            let output_num = self.camera.as_ref().output_num;
            assert_eq!(output_num, 3, "Expected camera to have 3 outputs");

            let preview_port_ptr =
                *(output.offset(MMAL_CAMERA_PREVIEW_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let video_port_ptr =
                *(output.offset(MMAL_CAMERA_VIDEO_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let still_port_ptr =
                *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let preview_port = *preview_port_ptr;
            let mut video_port = *video_port_ptr;
            let mut still_port = *still_port_ptr;

            // On firmware prior to June 2016, camera and video_splitter
            // had BGR24 and RGB24 support reversed.
            if encoding == ffi::MMAL_ENCODING_RGB24 || encoding == ffi::MMAL_ENCODING_BGR24 {
                encoding = if ffi::mmal_util_rgb_order_fixed(still_port_ptr) == 1 {
                    ffi::MMAL_ENCODING_RGB24
                } else {
                    ffi::MMAL_ENCODING_BGR24
                };
            }

            // TODO:
            //raspicamcontrol_set_all_parameters(camera, &state->camera_parameters);

            let mut format = preview_port.format;

            if self.use_encoder {
                (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            } else {
                (*format).encoding = encoding;
                (*format).encoding_variant = 0; //Irrelevant when not in opaque mode
            }
            // (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            let mut es = (*format).es;

            // Use a full FOV 4:3 mode
            (*es).video.width = ffi::vcos_align_up(1024, 32);
            (*es).video.height = ffi::vcos_align_up(768, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = 1024;
            (*es).video.crop.height = 768;
            (*es).video.frame_rate.num = PREVIEW_FRAME_RATE_NUM;
            (*es).video.frame_rate.den = PREVIEW_FRAME_RATE_DEN;

            let mut status = ffi::mmal_port_format_commit(preview_port_ptr);

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set preview port format".to_owned(),
                    status,
                ).into());
            }

            if video_port.buffer_num < VIDEO_OUTPUT_BUFFERS_NUM {
                video_port.buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
            }

            // Set the same format on the video port (which we don't use here)
            ffi::mmal_format_full_copy(video_port.format, preview_port.format);
            status = ffi::mmal_port_format_commit(video_port_ptr);

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set video port format".to_owned(),
                    status,
                ).into());
            }

            format = still_port.format;

            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L799

            if self.use_encoder {
                (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            } else {
                (*format).encoding = encoding;
                (*format).encoding_variant = 0; //Irrelevant when not in opaque mode
            }

            // (*still_port.format).encoding = ffi::MMAL_ENCODING_JPEG;
            // (*still_port.format).encoding_variant = ffi::MMAL_ENCODING_JPEG;

            // (*format).encoding = ffi::MMAL_ENCODING_I420;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;
            // (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            // es = elementary stream
            es = (*format).es;

            (*es).video.width = ffi::vcos_align_up(settings.width, 32);
            (*es).video.height = ffi::vcos_align_up(settings.height, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = settings.width as i32;
            (*es).video.crop.height = settings.height as i32;
            (*es).video.frame_rate.num = 0; //STILLS_FRAME_RATE_NUM;
            (*es).video.frame_rate.den = 1; //STILLS_FRAME_RATE_DEN;

            // TODO: should this be before or after the commit?
            if still_port.buffer_size < still_port.buffer_size_min {
                still_port.buffer_size = still_port.buffer_size_min;
            }

            still_port.buffer_num = still_port.buffer_num_recommended;

            let enable_zero_copy = if settings.zero_copy {
                ffi::MMAL_TRUE
            } else {
                ffi::MMAL_FALSE
            };
            status = ffi::mmal_port_parameter_set_boolean(
                video_port_ptr,
                ffi::MMAL_PARAMETER_ZERO_COPY as u32,
                enable_zero_copy as i32,
            );

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    format!("Unable to set zero copy to {}", settings.zero_copy),
                    status,
                ).into());
            }

            status = ffi::mmal_port_format_commit(still_port_ptr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set still port format".to_owned(),
                    status,
                ).into());
            }

            if !self.use_encoder {
                return Ok(());
            }

            let encoder_in_port_ptr =
                *(self.encoder.unwrap().as_ref().input.offset(0) as *mut *mut ffi::MMAL_PORT_T);
            let encoder_out_port_ptr =
                *(self.encoder.unwrap().as_ref().output.offset(0) as *mut *mut ffi::MMAL_PORT_T);
            let encoder_in_port = *encoder_in_port_ptr;
            let mut encoder_out_port = *encoder_out_port_ptr;

            // We want same format on input and output
            ffi::mmal_format_copy(encoder_out_port.format, encoder_in_port.format);

            format = encoder_out_port.format;
            (*format).encoding = encoding;

            encoder_out_port.buffer_size = encoder_out_port.buffer_size_recommended;
            if encoder_out_port.buffer_size < encoder_out_port.buffer_size_min {
                encoder_out_port.buffer_size = encoder_out_port.buffer_size_min;
            }

            encoder_out_port.buffer_num = encoder_out_port.buffer_num_recommended;
            if encoder_out_port.buffer_num < encoder_out_port.buffer_num_min {
                encoder_out_port.buffer_num = encoder_out_port.buffer_num_min;
            }

            status = ffi::mmal_port_format_commit(encoder_out_port_ptr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set encoder output port format".to_owned(),
                    status,
                ).into());
            }

            if encoding == ffi::MMAL_ENCODING_JPEG || encoding == ffi::MMAL_ENCODING_MJPEG {
                // Set the JPEG quality level
                status = ffi::mmal_port_parameter_set_uint32(
                    encoder_out_port_ptr,
                    ffi::MMAL_PARAMETER_JPEG_Q_FACTOR,
                    90,
                );
                if status != MMAL_STATUS_T::MMAL_SUCCESS {
                    return Err(MmalError::with_status(
                        "Unable to set JPEG quality".to_owned(),
                        status,
                    ).into());
                }

                // Set the JPEG restart interval
                status = ffi::mmal_port_parameter_set_uint32(
                    encoder_out_port_ptr,
                    ffi::MMAL_PARAMETER_JPEG_RESTART_INTERVAL,
                    0,
                );
                if status != MMAL_STATUS_T::MMAL_SUCCESS {
                    return Err(MmalError::with_status(
                        "Unable to set JPEG restart interval".to_owned(),
                        status,
                    ).into());
                }
            }

            // TODO: thumbnails
            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStill.c#L1290

            Ok(())
        }
    }

    pub fn enable(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_component_enable(self.camera.as_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.enabled = true;
                    Ok(())
                }
                s => Err(
                    MmalError::with_status("Unable to enable camera component".to_owned(), s)
                        .into(),
                ),
            }
        }
    }

    pub fn enable_encoder(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_port_enable(self.encoder.unwrap().as_ref().control, None);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.encoder_control_port_enabled = true;

                    let status = ffi::mmal_component_enable(self.encoder.unwrap().as_ptr());
                    match status {
                        MMAL_STATUS_T::MMAL_SUCCESS => {
                            self.encoder_enabled = true;
                            Ok(())
                        }
                        s => Err(MmalError::with_status(
                            "Unable to enable encoder component".to_owned(),
                            s,
                        ).into()),
                    }
                }
                s => Err(MmalError::with_status(
                    "Unable to enable encoder control port".to_owned(),
                    s,
                ).into()),
            }
        }
    }

    pub fn enable_preview(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_component_enable(&mut *self.preview.unwrap().as_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    // TODO: fix
                    // self.enabled = true;
                    Ok(())
                }
                s => Err(MmalError::with_status("Unable to enable preview".to_owned(), s).into()),
            }
        }
    }

    pub fn create_pool(&mut self) -> Result<(), CameraError> {
        unsafe {
            let port_ptr = if self.use_encoder {
                let output = self.encoder.unwrap().as_ref().output;
                *(output.offset(0) as *mut *mut ffi::MMAL_PORT_T)
            } else {
                let output = self.camera.as_ref().output;
                *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as *mut *mut ffi::MMAL_PORT_T)
            };

            let pool = ffi::mmal_port_pool_create(
                port_ptr,
                (*port_ptr).buffer_num,
                (*port_ptr).buffer_size,
            );

            if pool.is_null() {
                Err(MmalError::with_status(
                    format!(
                        "Failed to create buffer header pool for camera port {}",
                        CStr::from_ptr((*port_ptr).name).to_string_lossy()
                    ),
                    MMAL_STATUS_T::MMAL_STATUS_MAX, // there is no status here unusually
                ).into())
            } else {
                self.pool = Some(NonNull::new(pool).unwrap());
                Ok(())
            }
        }
    }

    pub fn create_preview(&mut self) -> Result<(), CameraError> {
        unsafe {
            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiPreview.c#L70
            // https://github.com/waveform80/picamera/issues/22
            // and the commit message that closed issue #22
            let mut preview_ptr: *mut ffi::MMAL_COMPONENT_T = mem::uninitialized();
            let status = ffi::mmal_component_create(
                ffi::MMAL_COMPONENT_NULL_SINK.as_ptr(),
                &mut preview_ptr,
            );

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.preview = Some(NonNull::new(&mut *preview_ptr).unwrap());
                    self.preview_created = true;
                    Ok(())
                }
                s => Err(MmalError::with_status(
                    "Unable to create null sink for preview".to_owned(),
                    s,
                ).into()),
            }
        }
    }

    pub fn connect_preview(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut connection_ptr: *mut ffi::MMAL_CONNECTION_T = mem::uninitialized();

            let preview_output_ptr = self.camera
                .as_ref()
                .output
                .offset(MMAL_CAMERA_PREVIEW_PORT as isize);
            let preview_input_ptr = self.preview.unwrap().as_ref().input.offset(0);

            let status = ffi::mmal_connection_create(
                &mut connection_ptr,
                *preview_output_ptr,
                *preview_input_ptr,
                ffi::MMAL_CONNECTION_FLAG_TUNNELLING
                    | ffi::MMAL_CONNECTION_FLAG_ALLOCATION_ON_INPUT,
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    // self.preview = Unique::new(&mut *preview_ptr);
                    // self.preview_created = true;
                    Ok(())
                }
                s => Err(
                    MmalError::with_status("Unable to connect preview ports".to_owned(), s).into(),
                ),
            }
        }
    }

    pub fn take(&mut self) -> Result<mpsc::Receiver<Option<BufferGuard>>, CameraError> {
        self.mutex.raw_lock();
        let buffer_port_ptr;

        unsafe {
            let mut status = ffi::mmal_port_parameter_set_uint32(
                self.camera.as_ref().control,
                ffi::MMAL_PARAMETER_SHUTTER_SPEED as u32,
                0, // 0 = auto
            );

            if status != ffi::MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set shutter speed".to_owned(),
                    status,
                ).into());
            }

            if self.use_encoder {
                if !self.encoder_output_port_enabled {
                    self.enable_encoder_port().unwrap();
                }
            } else {
                if !self.still_port_enabled {
                    self.enable_still_port().unwrap();
                }
            }

            // Send all the buffers to the camera output port
            let num = ffi::mmal_queue_length(self.pool.unwrap().as_ref().queue as *mut _);
            println!("got length {}", num);
            let output = self.camera.as_ref().output;

            let still_port_ptr =
                *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as *mut *mut ffi::MMAL_PORT_T);

            if self.use_encoder {
                let encoder_out_port_ptr =
                    *(self.encoder.unwrap().as_ref().output as *mut *mut ffi::MMAL_PORT_T);
                buffer_port_ptr = encoder_out_port_ptr;
            } else {
                buffer_port_ptr = still_port_ptr;
            }

            println!(
                "assigning pool of {} buffers size {}",
                (*buffer_port_ptr).buffer_num,
                (*buffer_port_ptr).buffer_size
            );

            for i in 0..num {
                let buffer = ffi::mmal_queue_get(self.pool.unwrap().as_ref().queue);
                println!("got buffer {}", i);

                if buffer.is_null() {
                    return Err(MmalError::with_status(
                        format!("Unable to get a required buffer {} from pool queue", i),
                        MMAL_STATUS_T::MMAL_STATUS_MAX,
                    ).into());
                } else {
                    status = ffi::mmal_port_send_buffer(buffer_port_ptr, buffer);
                    if status != MMAL_STATUS_T::MMAL_SUCCESS {
                        return Err(MmalError::with_status(
                            format!("Unable to send a buffer to camera output port ({})", i),
                            status,
                        ).into());
                    }
                }
            }

            let (sender, receiver) = mpsc::sync_channel(0);

            self.set_buffer_callback(sender);

            status = ffi::mmal_port_parameter_set_boolean(
                still_port_ptr,
                ffi::MMAL_PARAMETER_CAPTURE as u32,
                1,
            );

            // if self.use_encoder {
            //     status = ffi::mmal_port_parameter_set_boolean(buffer_port_ptr, ffi::MMAL_PARAMETER_EXIF_DISABLE, 1);
            // }

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    println!("Started capture");

                    Ok(receiver)
                }
                s => Err(MmalError::with_status(
                    "Unable to set camera capture boolean".to_owned(),
                    s,
                ).into()),
            }
        }.map_err(|e| {
            unsafe {
                if buffer_port_ptr != ptr::null_mut()
                    && (*buffer_port_ptr).userdata != ptr::null_mut()
                {
                    drop_port_userdata(buffer_port_ptr);
                }
            }
            e
        })
    }
}

unsafe extern "C" fn camera_buffer_callback(
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    let bytes_to_write = (*buffer).length;
    let pdata_ptr: *mut Userdata = (*port).userdata as *mut Userdata;
    let mut complete = false;

    println!("I'm called from C. buffer length: {}", bytes_to_write);

    if !pdata_ptr.is_null() {
        let userdata: &mut Userdata = &mut *pdata_ptr;

        // Check end of frame or error
        if ((*buffer).flags
            & (ffi::MMAL_BUFFER_HEADER_FLAG_FRAME_END
                | ffi::MMAL_BUFFER_HEADER_FLAG_TRANSMISSION_FAILED)) > 0
        {
            complete = true;
        }

        if bytes_to_write > 0 {
            ffi::mmal_buffer_header_mem_lock(buffer);

            userdata
                .sender
                .send(Some(BufferGuard::new(
                    port,
                    buffer,
                    userdata.pool,
                    complete,
                )))
                .unwrap();
        } else {
            if let Err(err) = userdata.sender.send(None) {
                println!("Got err sending None: {}", err);
            }
        }
    } else {
        println!("Received a camera still buffer callback with no state");
    }

    // println!("I'm done with c");
}

unsafe extern "C" fn camera_control_callback(
    _port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L525

    println!("Camera control callback  cmd=0x{:08x}", (*buffer).cmd);

    if (*buffer).cmd == ffi::MMAL_EVENT_PARAMETER_CHANGED {
        let param: *mut ffi::MMAL_EVENT_PARAMETER_CHANGED_T =
            (*buffer).data as *mut ffi::MMAL_EVENT_PARAMETER_CHANGED_T;
        if (*param).hdr.id == (ffi::MMAL_PARAMETER_CAMERA_SETTINGS as u32) {
            let settings_ptr: *mut ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T =
                param as *mut ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T;
            let settings: ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T = *settings_ptr;
            println!(
                "Exposure now {}, analog gain {}/{}, digital gain {}/{}",
                settings.exposure,
                settings.analog_gain.num,
                settings.analog_gain.den,
                settings.digital_gain.num,
                settings.digital_gain.den
            );
            println!(
                "AWB R={}/{}, B={}/{}",
                settings.awb_red_gain.num,
                settings.awb_red_gain.den,
                settings.awb_blue_gain.num,
                settings.awb_blue_gain.den
            );
        }
    } else if (*buffer).cmd == ffi::MMAL_EVENT_ERROR {
        println!(
            "No data received from sensor. Check all connections, including the Sunny one on the camera board"
        );
    } else {
        println!(
            "Received unexpected camera control callback event, {:08x}",
            (*buffer).cmd
        );
    }

    ffi::mmal_buffer_header_release(buffer);
}

impl Drop for SeriousCamera {
    fn drop(&mut self) {
        unsafe {
            let _guard = self.mutex.lock();

            if self.connection_created {
                ffi::mmal_connection_disable(self.connection.unwrap().as_ptr());
                ffi::mmal_connection_destroy(self.connection.unwrap().as_ptr());
            }
            if self.encoder_enabled {
                ffi::mmal_component_disable(self.encoder.unwrap().as_ptr());
                println!("encoder disabled");
            }
            if self.enabled {
                ffi::mmal_component_disable(self.camera.as_ptr());
                println!("camera disabled");
            }
            if self.encoder_control_port_enabled {
                ffi::mmal_port_disable(self.encoder.unwrap().as_ref().control);
                println!("port disabled");
            }
            if self.camera_port_enabled {
                ffi::mmal_port_disable(self.camera.as_ref().control);
                println!("port disabled");
            }

            ffi::mmal_component_destroy(self.camera.as_ptr());
            println!("camera destroyed");
            if self.encoder_created {
                ffi::mmal_component_destroy(self.encoder.unwrap().as_ptr());
                println!("encoder destroyed");
            }
        }
    }
}

/// A simple camera interface for the Raspberry Pi
///
/// # Examples
///
/// ```
/// use cam::SimpleCamera;
/// use std::fs::File;
/// use std::io::Write;
/// use std::{thread, time};
///
/// let info = cam::info().unwrap();
/// let mut camera = SimpleCamera::new(info.cameras[0].clone()).unwrap();
/// camera.activate().unwrap();
///
/// let sleep_duration = time::Duration::from_millis(2000);
/// thread::sleep(sleep_duration);
///
/// let b = camera.take_one().unwrap();
/// File::create("image1.jpg").unwrap().write_all(&b).unwrap();
/// ```
pub struct SimpleCamera {
    info: CameraInfo,
    serious: SeriousCamera,
    settings: Option<CameraSettings>,
}

impl SimpleCamera {
    pub fn new(info: CameraInfo) -> Result<SimpleCamera, CameraError> {
        let sc = SeriousCamera::new()?;

        Ok(SimpleCamera {
            info: info,
            serious: sc,
            settings: None,
        })
    }

    pub fn configure(&mut self, mut settings: CameraSettings) {
        if settings.width == 0 {
            settings.width = self.info.max_width;
        }
        if settings.height == 0 {
            settings.height = self.info.max_height;
        }

        self.settings = Some(settings);
    }

    pub fn activate(&mut self) -> Result<(), CameraError> {
        if self.settings.is_none() {
            self.configure(CameraSettings::default());
        }
        let settings = self.settings.as_ref().unwrap();
        let camera = &mut self.serious;

        camera.set_camera_num(0)?;
        camera.create_encoder()?;
        camera.set_camera_params(&self.info)?;

        camera.create_preview()?;

        // camera.set_camera_format(ffi::MMAL_ENCODING_JPEG, self.info.max_width, self.info.max_height, false)?;
        camera.set_camera_format(settings)?;
        camera.enable_control_port(false)?;

        camera.enable()?;
        camera.enable_encoder()?; // only needed if processing image eg returning jpeg
        camera.create_pool()?;

        camera.connect_preview()?;
        // camera.enable_preview()?;

        camera.connect_encoder()?;

        Ok(())
    }

    /// Captures a single image from the camera synchronously and writes it to the given `Write` trait.
    ///
    /// If there is an error
    pub fn take_one_writer(&mut self, writer: &mut ::std::io::Write) -> Result<(), CameraError> {
        let receiver = self.serious.take()?;

        loop {
            let result = receiver.recv()?;
            match result {
                Some(buf) => {
                    writer.write_all(buf.get_bytes())?;
                    if buf.is_complete() {
                        break;
                    }
                }
                None => break,
            };
        }

        Ok(())
    }

    /// Captures a single image from the camera synchronously.
    ///
    /// If successful then returns `Ok` with a `Vec<u8>` containing the bytes of the image.
    pub fn take_one(&mut self) -> Result<Vec<u8>, CameraError> {
        let mut v = Vec::new();
        self.take_one_writer(&mut v)?;
        Ok(v)
    }
}

/// Drops a port's userdata.
/// userdata must be non-null or will dereference a null pointer!
pub fn drop_port_userdata(port: *mut ffi::MMAL_PORT_T) {
    unsafe {
        let userdata: Box<Userdata> = Box::from_raw((*port).userdata as *mut Userdata);
        userdata._guard.raw_unlock();
        drop(userdata);
        (*port).userdata = ptr::null_mut() as *mut ffi::MMAL_PORT_USERDATA_T;
    }
}
