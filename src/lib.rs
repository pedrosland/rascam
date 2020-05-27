//! Access the native Raspberry Pi camera.
//!
//! This module uses the MMAL library ([mmal-sys]) to access the Raspberry Pi's camera
//! in a friendly way.
//!
//! [mmal-sys]: https://crates.io/crates/mmal-sys
#![allow(clippy::collapsible_if)]

use mmal_sys as ffi;
#[macro_use(defer_on_unwind)]
extern crate scopeguard;
use ffi::MMAL_STATUS_T;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use parking_lot::{lock_api::RawMutex, Mutex};
use std::ffi::CStr;
use std::io::Write;
use std::mem;
use std::mem::MaybeUninit;
use std::os::raw::c_char;
use std::ptr;
use std::ptr::NonNull;
use std::slice;
use std::sync::mpsc;
use std::sync::Arc;

mod error;
mod info;
mod init;
mod settings;

pub use error::{CameraError, MmalError};
pub use info::*;
use init::init;
pub use settings::*;

const MMAL_CAMERA_PREVIEW_PORT: isize = 0;
const MMAL_CAMERA_VIDEO_PORT: isize = 1;
const MMAL_CAMERA_CAPTURE_PORT: isize = 2;

const PREVIEW_FRAME_RATE_NUM: i32 = 0;
const PREVIEW_FRAME_RATE_DEN: i32 = 1;

/// Video needs at least 2 buffers.
const VIDEO_OUTPUT_BUFFERS_NUM: u32 = 3;

// TODO: what about the rest of these formats?
pub use ffi::MMAL_ENCODING_GIF;
pub use ffi::MMAL_ENCODING_JPEG;
pub use ffi::MMAL_ENCODING_PNG;

pub use ffi::MMAL_ENCODING_OPAQUE;

pub use ffi::MMAL_ENCODING_RGB24;

pub use ffi::MMAL_ENCODING_H264;

// TODO: do something about these:
pub use ffi::MMAL_VIDEO_LEVEL_H264_4;
pub use ffi::MMAL_VIDEO_LEVEL_H264_41;
pub use ffi::MMAL_VIDEO_LEVEL_H264_42;
pub use ffi::MMAL_VIDEO_PROFILE_H264_BASELINE;
pub use ffi::MMAL_VIDEO_PROFILE_H264_HIGH;

struct Userdata {
    pool: NonNull<ffi::MMAL_POOL_T>,
    _guard: Arc<Mutex<()>>,
    sender: SenderKind,
}

pub enum SenderKind {
    SyncSender(mpsc::SyncSender<Option<BufferGuard>>),
    AsyncSender(futures::channel::mpsc::Sender<BufferGuard>),
}

enum ReceiverKind {
    SyncReceiver(mpsc::Receiver<Option<BufferGuard>>),
    AsyncReceiver(futures::channel::mpsc::Receiver<BufferGuard>),
}

/// Guard around a buffer header.
///
/// Releases buffer header when it is dropped.
#[derive(Debug)]
pub struct BufferGuard {
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
    pool: NonNull<ffi::MMAL_POOL_T>,
    frame_end: bool,
}

impl BufferGuard {
    pub fn new(
        port: *mut ffi::MMAL_PORT_T,
        buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
        pool: NonNull<ffi::MMAL_POOL_T>,
        frame_end: bool,
    ) -> BufferGuard {
        BufferGuard {
            port,
            buffer,
            pool,
            frame_end,
        }
    }

    /// Indicates if an image has been captured and this is the end of the image.
    pub fn is_frame_end(&self) -> bool {
        self.frame_end
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
                    #[cfg(feature = "debug")]
                    println!("Unable to return the buffer to the port");
                }
            } else {
                // This might currently cause a hang. The hang is a bug but
                // the port being disabled is not a bug.
                #[cfg(feature = "debug")]
                println!("port disabled: cannot return buffer to disabled port");
            }
        }
    }
}

unsafe impl Send for BufferGuard {}

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
    is_capturing: bool,
    is_video: bool,
}

impl SeriousCamera {
    pub fn new() -> Result<SeriousCamera, CameraError> {
        init();
        unsafe {
            let mut camera_ptr = MaybeUninit::uninit();
            let component: *const c_char =
                ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const c_char;
            let status = ffi::mmal_component_create(component, camera_ptr.as_mut_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    let camera_ptr: *mut ffi::MMAL_COMPONENT_T = camera_ptr.assume_init();
                    Ok(SeriousCamera {
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
                        is_capturing: false,
                        is_video: false,
                    })
                }
                s => Err(MmalError::with_status("Could not create camera".to_owned(), s).into()),
            }
        }
    }

    pub fn set_camera_num(&mut self, num: u8) -> Result<(), CameraError> {
        unsafe {
            let mut param: ffi::MMAL_PARAMETER_INT32_T = mem::zeroed();
            param.hdr.id = ffi::MMAL_PARAMETER_CAMERA_NUM as u32;
            param.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_INT32_T>() as u32;
            param.value = num as i32;

            let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &param.hdr);
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
            let mut encoder_ptr = MaybeUninit::uninit();
            let component: *const c_char =
                ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const c_char;
            let status = ffi::mmal_component_create(component, encoder_ptr.as_mut_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    let encoder_ptr: *mut ffi::MMAL_COMPONENT_T = encoder_ptr.assume_init();
                    self.encoder = Some(NonNull::new(encoder_ptr).unwrap());
                    self.encoder_created = true;
                    Ok(())
                }
                s => Err(MmalError::with_status("Unable to create encoder".to_owned(), s).into()),
            }
        }
    }

    pub fn create_video_encoder(&mut self) -> Result<(), CameraError> {
        self.is_video = true;

        unsafe {
            let mut encoder_ptr = MaybeUninit::uninit();
            let component: *const c_char =
                ffi::MMAL_COMPONENT_DEFAULT_VIDEO_ENCODER.as_ptr() as *const c_char;
            let status = ffi::mmal_component_create(component, encoder_ptr.as_mut_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    let encoder_ptr: *mut ffi::MMAL_COMPONENT_T = encoder_ptr.assume_init();
                    self.encoder = Some(NonNull::new(encoder_ptr).unwrap());
                    self.encoder_created = true;
                    Ok(())
                }
                s => Err(
                    MmalError::with_status("Unable to create video encoder".to_owned(), s).into(),
                ),
            }
        }
    }

    pub fn connect_encoder(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut connection_ptr = MaybeUninit::uninit();
            let port = if self.is_video {
                *self.camera.as_ref().output.offset(MMAL_CAMERA_VIDEO_PORT)
            } else {
                *self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT)
            };

            let status = ffi::mmal_connection_create(
                connection_ptr.as_mut_ptr(),
                port,
                *self.encoder.unwrap().as_ref().input.offset(0),
                ffi::MMAL_CONNECTION_FLAG_TUNNELLING
                    | ffi::MMAL_CONNECTION_FLAG_ALLOCATION_ON_INPUT,
            );
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to create camera->encoder connection".to_owned(),
                    status,
                )
                .into());
            }

            let connection_ptr: *mut ffi::MMAL_CONNECTION_T = connection_ptr.assume_init();
            self.connection = Some(NonNull::new(connection_ptr).unwrap());
            self.connection_created = true;
            let status = ffi::mmal_connection_enable(&mut *connection_ptr);

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => Err(MmalError::with_status(
                    "Unable to enable camera->encoder connection".to_owned(),
                    s,
                )
                .into()),
            }
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

    /// Set callback function to be called when there is data from the camera.
    ///
    /// # Safety
    /// This function will be passed to C so you are responsible for it.
    /// Make no assumptions about when this will be called or what thread it will be called from.
    pub unsafe fn set_buffer_callback(&mut self, sender: SenderKind) {
        let port = if self.use_encoder {
            *self.encoder.unwrap().as_ref().output.offset(0)
        } else {
            *self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT)
        };

        let userdata = Userdata {
            pool: self.pool.unwrap(),
            sender,
            _guard: Arc::clone(&self.mutex),
        };

        if !(*port).userdata.is_null() {
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

    pub fn set_camera_params(&mut self, info: &CameraInfo, one_shot_stills: bool, framerate: u32) -> Result<(), CameraError> {
        unsafe {
            let mut cfg: ffi::MMAL_PARAMETER_CAMERA_CONFIG_T = mem::zeroed();
            cfg.hdr.id = ffi::MMAL_PARAMETER_CAMERA_CONFIG as u32;
            cfg.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_CONFIG_T>() as u32;

            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L706
            cfg.max_stills_w = info.max_width;
            cfg.max_stills_h = info.max_height;
            cfg.stills_yuv422 = 0;
            cfg.one_shot_stills = if one_shot_stills { 1 } else { 0 };
            cfg.max_preview_video_w = info.max_width;
            cfg.max_preview_video_h = info.max_height;
            cfg.num_preview_video_frames = 3 + std::cmp::max(0, (framerate as i32-30)/10) as u32;
            cfg.stills_capture_circular_buffer_height = 0;
            cfg.fast_preview_resume = 0;
            cfg.use_stc_timestamp = ffi::MMAL_PARAMETER_CAMERA_CONFIG_TIMESTAMP_MODE_T::MMAL_PARAM_TIMESTAMP_MODE_RESET_STC;

            let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &cfg.hdr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => Err(MmalError::with_status(
                    "Unable to set control port parmaeter".to_owned(),
                    s,
                )
                .into()),
            }
        }
    }

    pub fn set_video_camera_format(
        &mut self,
        settings: &CameraSettings,
    ) -> Result<(), CameraError> {
        unsafe {
            self.use_encoder = settings.use_encoder;
            let encoding = settings.encoding;

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
            let video_port = *video_port_ptr;
            let still_port = *still_port_ptr;

            let control = self.camera.as_ref().control;

            // TODO:
            //raspicamcontrol_set_all_parameters(camera, &state->camera_parameters);

            let status =
                ffi::mmal_port_parameter_set_uint32(control, ffi::MMAL_PARAMETER_ISO, settings.iso);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status("Unable to set ISO".to_owned(), status).into());
            }

            let mut format = preview_port.format;

            (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            // TODO: MMAL_PARAMETER_FPS_RANGE
            // TODO: dynamic frame rate

            // es = elementary stream
            let mut es = (*format).es;

            // Use same dimensions as the video
            (*es).video.width = ffi::vcos_align_up(settings.width, 32);
            (*es).video.height = ffi::vcos_align_up(settings.height, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = settings.width as i32;
            (*es).video.crop.height = settings.height as i32;
            (*es).video.frame_rate.num = settings.framerate as i32;
            (*es).video.frame_rate.den = PREVIEW_FRAME_RATE_DEN;

            let mut status = ffi::mmal_port_format_commit(preview_port_ptr);

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set preview port format".to_owned(),
                    status,
                )
                .into());
            }

            format = video_port.format;

            (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            es = (*format).es;

            // This should be a copy of the above.
            // Note that the format copy function doesn't seem to copy everything???
            (*es).video.width = ffi::vcos_align_up(settings.width, 32);
            (*es).video.height = ffi::vcos_align_up(settings.height, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = settings.width as i32;
            (*es).video.crop.height = settings.height as i32;
            (*es).video.frame_rate.num = settings.framerate as i32;
            (*es).video.frame_rate.den = PREVIEW_FRAME_RATE_DEN;

            if video_port.buffer_num < VIDEO_OUTPUT_BUFFERS_NUM {
                (*video_port_ptr).buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
            }
            status = ffi::mmal_port_format_commit(video_port_ptr);

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set video port format".to_owned(),
                    status,
                )
                .into());
            }

            format = still_port.format;

            (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            es = (*format).es;

            (*es).video.width = ffi::vcos_align_up(settings.width, 32);
            (*es).video.height = ffi::vcos_align_up(settings.height, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = settings.width as i32;
            (*es).video.crop.height = settings.height as i32;
            (*es).video.frame_rate.num = 0;
            (*es).video.frame_rate.den = 1;

            // TODO: should this be before or after the commit?
            if still_port.buffer_size < still_port.buffer_size_min {
                (*still_port_ptr).buffer_size = still_port.buffer_size_min;
            }

            (*still_port_ptr).buffer_num = still_port.buffer_num_recommended;

            if still_port.buffer_num < VIDEO_OUTPUT_BUFFERS_NUM {
                (*still_port_ptr).buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
            }

            status = ffi::mmal_port_format_commit(still_port_ptr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set still port format".to_owned(),
                    status,
                )
                .into());
            }

            if !self.use_encoder {
                return Ok(());
            }

            let encoder_in_port_ptr =
                *(self.encoder.unwrap().as_ref().input.offset(0) as *mut *mut ffi::MMAL_PORT_T);
            let encoder_out_port_ptr =
                *(self.encoder.unwrap().as_ref().output.offset(0) as *mut *mut ffi::MMAL_PORT_T);
            let encoder_in_port = *encoder_in_port_ptr;
            let encoder_out_port = *encoder_out_port_ptr;

            (*(*encoder_out_port.format).es).video.frame_rate.num = 1_966_080;

            // We want same format on input and output
            ffi::mmal_format_copy(encoder_out_port.format, encoder_in_port.format);

            format = encoder_out_port.format;
            (*format).encoding = encoding;
            (*format).bitrate = 17_000_000;

            es = (*format).es;

            // We need to set the frame rate on output to 0, to ensure it gets
            // updated correctly from the input framerate when port connected
            (*es).video.frame_rate.num = 1_966_080;
            (*es).video.frame_rate.den = 1;
            (*es).video.height = 1080;

            if encoding == MMAL_ENCODING_H264 {
                (*encoder_out_port_ptr).buffer_size = encoder_out_port.buffer_size_recommended;
            } else {
                (*encoder_out_port_ptr).buffer_size = 256 << 10
            }
            if encoder_out_port.buffer_size < encoder_out_port.buffer_size_min {
                (*encoder_out_port_ptr).buffer_size = encoder_out_port.buffer_size_min;
            }

            (*encoder_out_port_ptr).buffer_num = encoder_out_port.buffer_num_recommended;

            status = ffi::mmal_port_format_commit(encoder_out_port_ptr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set encoder output port format".to_owned(),
                    status,
                )
                .into());
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
                    )
                    .into());
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
                    )
                    .into());
                }
            }

            // Various h264 settings
            if encoding == MMAL_ENCODING_H264 {
                self.set_h264_settings(encoder_out_port_ptr, &settings)?;
            }

            Ok(())
        }
    }

    fn set_h264_settings(&mut self, encoder_out_port_ptr: *mut ffi::MMAL_PORT_T, settings: &CameraSettings) -> Result<(), CameraError> {
        unsafe {
            let mut param: ffi::MMAL_PARAMETER_VIDEO_PROFILE_T = mem::zeroed();
            param.hdr.id = ffi::MMAL_PARAMETER_PROFILE as u32;
            param.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_VIDEO_PROFILE_T>() as u32;

            param.profile[0].profile = settings.video_profile;
            param.profile[0].level = settings.video_level;

            let status = ffi::mmal_port_parameter_set(encoder_out_port_ptr, &param.hdr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status("Unable to set h264 settings".to_owned(), status).into());
            }

            Ok(())

            // TODO: Check if there are enough macroblocks somewhere

            // TODO: more H264 encoding settings
            // eg MMAL_PARAMETER_INTRAPERIOD, MMAL_PARAMETER_MB_ROWS_PER_SLICE, MMAL_PARAMETER_VIDEO_ENCODE_INITIAL_QUANT, MMAL_PARAMETER_VIDEO_ENCODE_MIN_QUANT, MMAL_PARAMETER_VIDEO_ENCODE_MAX_QUANT
            // MMAL_PARAMETER_VIDEO_IMMUTABLE_INPUT, MMAL_PARAMETER_VIDEO_ENCODE_INLINE_HEADER, MMAL_PARAMETER_VIDEO_ENCODE_SPS_TIMING, MMAL_PARAMETER_VIDEO_ENCODE_INLINE_VECTORS, MMAL_PARAMETER_VIDEO_INTRA_REFRESH_T
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

            let control = self.camera.as_ref().control;

            // TODO:
            //raspicamcontrol_set_all_parameters(camera, &state->camera_parameters);

            let status =
                ffi::mmal_port_parameter_set_uint32(control, ffi::MMAL_PARAMETER_ISO, settings.iso);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status("Unable to set ISO".to_owned(), status).into());
            }

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
                )
                .into());
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
                )
                .into());
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
                )
                .into());
            }

            status = ffi::mmal_port_format_commit(still_port_ptr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(MmalError::with_status(
                    "Unable to set still port format".to_owned(),
                    status,
                )
                .into());
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
                )
                .into());
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
                    )
                    .into());
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
                    )
                    .into());
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
                        )
                        .into()),
                    }
                }
                s => Err(MmalError::with_status(
                    "Unable to enable encoder control port".to_owned(),
                    s,
                )
                .into()),
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

            #[cfg(feature = "debug")]
            println!("create pool buffer_num: {}, buffer_size: {}", (*port_ptr).buffer_num, (*port_ptr).buffer_size);

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
                )
                .into())
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
            let mut preview_ptr = MaybeUninit::uninit();
            let status = ffi::mmal_component_create(
                ffi::MMAL_COMPONENT_NULL_SINK.as_ptr(),
                preview_ptr.as_mut_ptr(),
            );

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    let preview_ptr: *mut ffi::MMAL_COMPONENT_T = preview_ptr.assume_init();
                    self.preview = Some(NonNull::new(&mut *preview_ptr).unwrap());
                    self.preview_created = true;
                    Ok(())
                }
                s => Err(MmalError::with_status(
                    "Unable to create null sink for preview".to_owned(),
                    s,
                )
                .into()),
            }
        }
    }

    pub fn connect_preview(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut connection_ptr = MaybeUninit::uninit();

            let preview_output_ptr = self
                .camera
                .as_ref()
                .output
                .offset(MMAL_CAMERA_PREVIEW_PORT as isize);
            let preview_input_ptr = self.preview.unwrap().as_ref().input.offset(0);

            let status = ffi::mmal_connection_create(
                connection_ptr.as_mut_ptr(),
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

    unsafe fn send_buffers(
        &mut self,
        buffer_port_ptr: *mut ffi::MMAL_PORT_T,
    ) -> Result<(), CameraError> {
        let num = ffi::mmal_queue_length(self.pool.unwrap().as_ref().queue as *mut _);
        #[cfg(feature = "debug")]
        println!("got length {}", num);

        #[cfg(feature = "debug")]
        println!(
            "assigning pool of {} buffers size {}",
            (*buffer_port_ptr).buffer_num,
            (*buffer_port_ptr).buffer_size
        );

        for i in 0..num {
            let buffer = ffi::mmal_queue_get(self.pool.unwrap().as_ref().queue);
            #[cfg(feature = "debug")]
            println!("got buffer {}", i);

            if buffer.is_null() {
                return Err(MmalError::with_status(
                    format!("Unable to get a required buffer {} from pool queue", i),
                    MMAL_STATUS_T::MMAL_STATUS_MAX,
                )
                .into());
            } else {
                let status = ffi::mmal_port_send_buffer(buffer_port_ptr, buffer);
                if status != MMAL_STATUS_T::MMAL_SUCCESS {
                    return Err(MmalError::with_status(
                        format!("Unable to send a buffer to camera output port ({})", i),
                        status,
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    fn do_take(
        &mut self,
        buffer_port_ptr: &mut *mut ffi::MMAL_PORT_T,
        is_async: bool,
    ) -> Result<ReceiverKind, CameraError> {
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
                )
                .into());
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

            let output = self.camera.as_ref().output;

            let camera_port_ptr = if self.is_video {
                *(output.offset(MMAL_CAMERA_VIDEO_PORT) as *mut *mut ffi::MMAL_PORT_T)
            } else {
                *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as *mut *mut ffi::MMAL_PORT_T)
            };

            if self.use_encoder {
                let encoder_out_port_ptr =
                    *(self.encoder.unwrap().as_ref().output as *mut *mut ffi::MMAL_PORT_T);
                *buffer_port_ptr = encoder_out_port_ptr;
            } else {
                *buffer_port_ptr = camera_port_ptr;
            }

            let (sender, receiver) = if is_async {
                let (sender, receiver) = futures::channel::mpsc::channel(0);
                (
                    SenderKind::AsyncSender(sender),
                    ReceiverKind::AsyncReceiver(receiver),
                )
            } else {
                let (sender, receiver) = mpsc::sync_channel(0);
                (
                    SenderKind::SyncSender(sender),
                    ReceiverKind::SyncReceiver(receiver),
                )
            };

            self.set_buffer_callback(sender);

            // Send all the buffers to the camera output port
            self.send_buffers(*buffer_port_ptr)?;

            status = ffi::mmal_port_parameter_set_boolean(
                camera_port_ptr,
                ffi::MMAL_PARAMETER_CAPTURE as u32,
                1,
            );

            // if self.use_encoder {
            //     status = ffi::mmal_port_parameter_set_boolean(buffer_port_ptr, ffi::MMAL_PARAMETER_EXIF_DISABLE, 1);
            // }

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    #[cfg(feature = "debug")]
                    println!("Started capture");
                    self.is_capturing = true;

                    Ok(receiver)
                }
                s => Err(MmalError::with_status(
                    "Unable to set camera capture boolean".to_owned(),
                    s,
                )
                .into()),
            }
        }
    }

    pub fn take(&mut self) -> Result<mpsc::Receiver<Option<BufferGuard>>, CameraError> {
        unsafe {
            self.mutex.raw().lock();
        }

        let mut buffer_port_ptr = ptr::null_mut();
        let mutex = Arc::clone(&self.mutex);

        defer_on_unwind! {{
            unsafe { mutex.force_unlock() };
        }}

        self.do_take(&mut buffer_port_ptr, false)
            .map_err(|e| {
                unsafe {
                    if !buffer_port_ptr.is_null() && !(*buffer_port_ptr).userdata.is_null() {
                        drop_port_userdata(buffer_port_ptr);
                    }
                    self.mutex.force_unlock();
                }
                e
            })
            .map(|receiver| match receiver {
                ReceiverKind::SyncReceiver(receiver) => receiver,
                ReceiverKind::AsyncReceiver(_) => unreachable!(),
            })
    }

    pub fn take_async(
        &mut self,
    ) -> Result<futures::channel::mpsc::Receiver<BufferGuard>, CameraError> {
        unsafe {
            self.mutex.raw().lock();
        }

        let mut buffer_port_ptr = ptr::null_mut();
        let mutex = Arc::clone(&self.mutex);

        defer_on_unwind! {{
            unsafe { mutex.force_unlock() };
        }}

        self.do_take(&mut buffer_port_ptr, true)
            .map_err(|e| {
                unsafe {
                    if buffer_port_ptr.is_null() && (*buffer_port_ptr).userdata.is_null() {
                        drop_port_userdata(buffer_port_ptr);
                    }
                    self.mutex.force_unlock();
                }
                e
            })
            .map(|receiver| match receiver {
                ReceiverKind::AsyncReceiver(receiver) => receiver,
                ReceiverKind::SyncReceiver(_) => unreachable!(),
            })
    }

    /// Stop capturing video or images.
    ///
    /// # Safety
    ///
    /// This function is always safe to call, even if you are not capturing.
    pub fn stop_capturing(&mut self) {
        if !self.is_capturing {
            return;
        }

        let offset = if self.is_video {
            MMAL_CAMERA_VIDEO_PORT
        } else {
            MMAL_CAMERA_CAPTURE_PORT
        };

        unsafe {
            let port_ptr = *(self.camera.as_ref().output.offset(offset) as *mut *mut ffi::MMAL_PORT_T);
            ffi::mmal_port_parameter_set_boolean(
                port_ptr,
                ffi::MMAL_PARAMETER_CAPTURE as u32,
                0,
            );
        }

        self.is_capturing = false;
        #[cfg(feature = "debug")]
        println!("stopped capturing");
    }
}

#[allow(clippy::let_unit_value)]
unsafe extern "C" fn camera_buffer_callback(
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    let bytes_to_write = (*buffer).length;
    #[allow(clippy::cast_ptr_alignment)]
    let pdata_ptr: *mut Userdata = (*port).userdata as *mut Userdata;
    let mut frame_end = false;

    #[cfg(feature = "debug")]
    println!("camera_buffer_callback() port name: {:?} buffer length: {} flags: {}", CStr::from_ptr((*port).name), bytes_to_write, (*buffer).flags);

    // for video, first we have flags 4132 then 4108. Possibly a 3rd time with 4100.
    // 4132 =
    // pub const MMAL_BUFFER_HEADER_FLAG_NAL_END: u32 = 4096;
    // pub const MMAL_BUFFER_HEADER_FLAG_CONFIG: u32 = 32;
    // pub const MMAL_BUFFER_HEADER_FLAG_FRAME_END: u32 = 4;
    //
    // 4108 =
    // pub const MMAL_BUFFER_HEADER_FLAG_NAL_END: u32 = 4096;
    // pub const MMAL_BUFFER_HEADER_FLAG_KEYFRAME: u32 = 8;
    // pub const MMAL_BUFFER_HEADER_FLAG_FRAME_END: u32 = 4;
    //
    // 4100 =
    // pub const MMAL_BUFFER_HEADER_FLAG_NAL_END: u32 = 4096;
    // pub const MMAL_BUFFER_HEADER_FLAG_FRAME_END: u32 = 4;

    if !pdata_ptr.is_null() {
        let userdata: &mut Userdata = &mut *pdata_ptr;

        // Check end of frame or error
        if ((*buffer).flags
            & (ffi::MMAL_BUFFER_HEADER_FLAG_FRAME_END
                | ffi::MMAL_BUFFER_HEADER_FLAG_TRANSMISSION_FAILED))
            > 0
        {
            frame_end = true;
        }

        if bytes_to_write > 0 {
            ffi::mmal_buffer_header_mem_lock(buffer);

            match &mut userdata.sender {
                SenderKind::AsyncSender(sender) => {
                    sender
                        .try_send(BufferGuard::new(port, buffer, userdata.pool, frame_end))
                        .unwrap();
                }
                SenderKind::SyncSender(sender) => {
                    let result = sender
                        .send(Some(BufferGuard::new(
                            port,
                            buffer,
                            userdata.pool,
                            frame_end,
                        )));
                    if let Err(_err) = result {
                        #[cfg(feature = "debug")]
                        println!("Got err sending data to channel: {:?}", _err);
                    }
                }
            }
        } else {
            // Release buffer back to the pool
            ffi::mmal_buffer_header_release(buffer);

            match &mut userdata.sender {
                SenderKind::AsyncSender(sender) => sender.close_channel(),
                SenderKind::SyncSender(sender) => {
                    if let Err(_err) = sender.send(None) {
                        #[cfg(feature = "debug")]
                        println!("Got err sending None to channel: {}", _err);
                    }
                }
            }

            if !(*port).userdata.is_null() {
                drop_port_userdata(port);
            }

            // Get new buffer from the pool and send it to the port (if still open)
            if (*port).is_enabled > 0 {
                let mut status = ffi::MMAL_STATUS_T::MMAL_STATUS_MAX;
                let new_buffer: *mut ffi::MMAL_BUFFER_HEADER_T =
                    ffi::mmal_queue_get(userdata.pool.as_ref().queue);

                if !new_buffer.is_null() {
                    status = ffi::mmal_port_send_buffer(port, new_buffer);
                }

                if new_buffer.is_null() || status != MMAL_STATUS_T::MMAL_SUCCESS {
                    #[cfg(feature = "debug")]
                    println!("Unable to return the buffer to the port");
                }
            }
        }
    } else {
        #[cfg(feature = "debug")]
        println!("Received a camera still buffer callback with no state");

        // Release buffer back to the pool
        ffi::mmal_buffer_header_release(buffer);
    }
}

#[allow(clippy::if_same_then_else)]
unsafe extern "C" fn camera_control_callback(
    _port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L525

    #[cfg(feature = "debug")]
    println!("Camera control callback cmd=0x{:08x}", (*buffer).cmd);

    if (*buffer).cmd == ffi::MMAL_EVENT_PARAMETER_CHANGED {
        #[allow(clippy::cast_ptr_alignment)]
        let param: *mut ffi::MMAL_EVENT_PARAMETER_CHANGED_T =
            (*buffer).data as *mut ffi::MMAL_EVENT_PARAMETER_CHANGED_T;
        if (*param).hdr.id == (ffi::MMAL_PARAMETER_CAMERA_SETTINGS as u32) {
            let settings_ptr: *mut ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T =
                param as *mut ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T;
            let _settings: ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T = *settings_ptr;
            #[cfg(feature = "debug")]
            println!(
                "Exposure now {}, analog gain {}/{}, digital gain {}/{}",
                _settings.exposure,
                _settings.analog_gain.num,
                _settings.analog_gain.den,
                _settings.digital_gain.num,
                _settings.digital_gain.den
            );
            #[cfg(feature = "debug")]
            println!(
                "AWB R={}/{}, B={}/{}",
                _settings.awb_red_gain.num,
                _settings.awb_red_gain.den,
                _settings.awb_blue_gain.num,
                _settings.awb_blue_gain.den
            );
        }
    } else if (*buffer).cmd == ffi::MMAL_EVENT_ERROR {
        #[cfg(feature = "debug")]
        println!(
            "No data received from sensor. Check all connections, including the Sunny one on the camera board"
        );
    } else {
        #[cfg(feature = "debug")]
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
            let was_capturing = self.is_capturing;
            self.stop_capturing();

            if self.encoder_output_port_enabled {
                let port = *self.encoder.unwrap().as_ref().output.offset(0);
                ffi::mmal_port_disable(port);
                #[cfg(feature = "debug")]
                println!("encoder output port disabled");
            }

            if was_capturing && !self.use_encoder {
                if self.is_video {
                    ffi::mmal_port_disable(*(self.camera.as_ref().output.offset(MMAL_CAMERA_VIDEO_PORT)));
                    #[cfg(feature = "debug")]
                    println!("camera video port port disabled");
                } else {
                    ffi::mmal_port_disable(*(self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT)));
                    #[cfg(feature = "debug")]
                    println!("camera still port port disabled");
                }
            }

            if self.camera_port_enabled {
                ffi::mmal_port_disable(self.camera.as_ref().control);
                #[cfg(feature = "debug")]
                println!("camera control port disabled");
            }
            if self.encoder_control_port_enabled {
                ffi::mmal_port_disable(self.encoder.unwrap().as_ref().control);
                #[cfg(feature = "debug")]
                println!("encoder control port disabled");
            }

            if self.connection_created {
                ffi::mmal_connection_disable(self.connection.unwrap().as_ptr());
                ffi::mmal_connection_destroy(self.connection.unwrap().as_ptr());
            }

            if self.encoder_enabled {
                ffi::mmal_component_disable(self.encoder.unwrap().as_ptr());
                #[cfg(feature = "debug")]
                println!("encoder disabled");
            }
            if self.enabled {
                ffi::mmal_component_disable(self.camera.as_ptr());
                #[cfg(feature = "debug")]
                println!("camera disabled");
            }

            ffi::mmal_component_destroy(self.camera.as_ptr());
            #[cfg(feature = "debug")]
            println!("camera destroyed");
            if self.encoder_created {
                ffi::mmal_component_destroy(self.encoder.unwrap().as_ptr());
                #[cfg(feature = "debug")]
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
/// use rascam::SimpleCamera;
/// use std::fs::File;
/// use std::io::Write;
/// use std::{thread, time};
///
/// let info = rascam::info().unwrap();
/// let mut camera = SimpleCamera::new(info.cameras[0].clone()).unwrap();
/// camera.activate().unwrap();
///
/// let sleep_duration = time::Duration::from_millis(2000);
/// thread::sleep(sleep_duration);
///
/// let b = camera.take_one().unwrap();
/// File::create("image.jpg").unwrap().write_all(&b).unwrap();
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
            info,
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

        let one_shot_stills = settings.encoding != ffi::MMAL_ENCODING_H264;

        camera.set_camera_num(0)?;
        if settings.encoding == MMAL_ENCODING_H264 {
            camera.create_video_encoder()?;
        } else {
            camera.create_encoder()?;
        }
        camera.enable_control_port(false)?;
        camera.set_camera_params(&self.info, one_shot_stills, settings.framerate)?;

        camera.create_preview()?;

        // camera.set_camera_format(ffi::MMAL_ENCODING_JPEG, self.info.max_width, self.info.max_height, false)?;
        if settings.encoding == MMAL_ENCODING_H264 {
            camera.set_video_camera_format(settings)?;
        } else {
            camera.set_camera_format(settings)?;
        }

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
    pub fn take_one_writer(&mut self, writer: &mut dyn Write) -> Result<(), CameraError> {
        let receiver = self.serious.take()?;

        loop {
            let result = receiver.recv()?;
            match result {
                Some(buf) => {
                    writer.write_all(buf.get_bytes())?;
                    if buf.is_frame_end() {
                        break;
                    }
                }
                None => break,
            }
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

    /// Captures a single image from the camera asynchronously.
    ///
    /// Returns a future result where `Ok` contains a `Vec<u8>` containing the bytes of the image.
    pub async fn take_one_async(&mut self) -> Result<Vec<u8>, CameraError> {
        let receiver = self.serious.take_async()?;
        let future = receiver
            .fold(Vec::new(), |mut acc, buf| async move {
                acc.extend(buf.get_bytes());
                acc
            })
            .map(Ok);

        future.await
    }

    /// Starts capturing video and returns an iterator of frames.
    pub fn take_video_frame_writer(&mut self) -> Result<impl std::iter::Iterator<Item=Vec<u8>>, CameraError> {
        let mut frame = Vec::new();
        let receiver = self.serious.take()?;

        Ok(receiver
            .into_iter()
            .take_while(|buf| {
                buf.is_some()
            })
            .filter_map(move |buf| {
                let buf = buf.unwrap();

                frame.extend(buf.get_bytes());
                if buf.is_frame_end() {
                    let result = Some(frame.clone());
                    frame.truncate(0);
                    result
                } else {
                    None
                }
            }))
    }

    /// Stops capturing.
    ///
    /// This is safe to call regardless of if there is any capture in progress.
    pub fn stop(mut self) {
        self.serious.stop_capturing();
    }
}


/// Drops a port's userdata.
///
/// # Safety
///
/// `port.userdata` must be non-null or this will dereference a null pointer.
#[allow(clippy::cast_ptr_alignment)]
pub unsafe fn drop_port_userdata(port: *mut ffi::MMAL_PORT_T) {
    let userdata: Box<Userdata> = Box::from_raw((*port).userdata as *mut Userdata);
    userdata._guard.force_unlock();
    drop(userdata);
    (*port).userdata = ptr::null_mut() as *mut ffi::MMAL_PORT_USERDATA_T;
    #[cfg(feature = "debug")]
    println!("dropped port userdata. port.name: {}", CStr::from_ptr((*port).name).to_str().unwrap());
}

trait Sender {
    fn try_send(&mut self, msg: BufferGuard);
}

impl Sender for futures::channel::mpsc::Sender<BufferGuard> {
    fn try_send(&mut self, msg: BufferGuard) {
        futures::channel::mpsc::Sender::try_send(self, msg).unwrap()
    }
}
