#![feature(unique)]
extern crate libc;
extern crate mmal_sys as ffi;
// extern crate futures;
use ffi::MMAL_STATUS_T;
use std::fmt;
use std::os::raw::c_char;
use std::mem;
use std::ptr::Unique;
use std::fs::File;
use std::slice;
use std::string::String;
use std::sync::{Once, ONCE_INIT};
use std::sync::mpsc;
use std::os::raw::c_uint;

pub use error::CameraError;

mod error;

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

pub struct Info {
    pub cameras: Vec<CameraInfo>,
    // TODO: flashes?
}

impl fmt::Display for Info {
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

#[derive(Clone, Debug)]
pub struct CameraInfo {
    pub port_id: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub lens_present: bool,
    pub camera_name: String,
}

impl fmt::Display for CameraInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {}x{}",
            &self.camera_name, self.max_width, self.max_height
        )
    }
}

// This function must be called before any mmal work. Failure to do so will cause errors like:
//
// mmal: mmal_component_create_core: could not find component 'vc.camera_info'
//
// See this for more info https://github.com/thaytan/gst-rpicamsrc/issues/28
fn init() {
    static INIT: Once = ONCE_INIT;
    INIT.call_once(|| unsafe {
        ffi::bcm_host_init();
        ffi::vcos_init();
        ffi::mmal_vc_init();
    });
}

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
                                camera_name: ::std::str::from_utf8(&cam.camera_name)
                                    .unwrap()
                                    .to_owned(),
                            })
                            .collect();

                        ffi::mmal_component_destroy(component);

                        Ok(Info { cameras: cameras })
                    }
                    s => {
                        ffi::mmal_component_destroy(component);
                        Err(CameraError::with_status("Failed to get camera info", s))
                    }
                }
            }
            s => Err(CameraError::with_status(
                "Failed to create camera component",
                s,
            )),
        }
    }
}

struct Userdata {
    pool: Unique<ffi::MMAL_POOL_T>,
    callback: Box<Fn(Option<&ffi::MMAL_BUFFER_HEADER_T>)>,
}

#[repr(C)]
pub struct SeriousCamera {
    camera: Unique<ffi::MMAL_COMPONENT_T>,
    outputs: Vec<ffi::MMAL_PORT_T>,
    enabled: bool,
    camera_port_enabled: bool,
    still_port_enabled: bool,
    pool: Unique<ffi::MMAL_POOL_T>,

    encoder: Unique<ffi::MMAL_COMPONENT_T>,
    encoder_created: bool,
    encoder_enabled: bool,
    encoder_control_port_enabled: bool,
    encoder_output_port_enabled: bool,

    connection: Unique<ffi::MMAL_CONNECTION_T>,
    connection_created: bool,

    preview: Unique<ffi::MMAL_COMPONENT_T>,
    preview_created: bool,

    file: File,
    file_open: bool,
    // FnOnce
    // buffer_callback: Box<Fn(Option<&ffi::MMAL_BUFFER_HEADER_T>) + 'static>,

    use_encoder: bool,
}

impl SeriousCamera {
    pub fn new() -> Result<SeriousCamera, ffi::MMAL_STATUS_T::Type> {
        init();
        /*

            component_type = mmal.MMAL_COMPONENT_DEFAULT_CAMERA
        opaque_output_subformats = ('OPQV-single', 'OPQV-dual', 'OPQV-strips')

        mmal.mmal_component_create(self.component_type, self._component),
            prefix="Failed to create MMAL component %s" % self.component_type)
            */

        /* Useful if mmal_component_create were to take a pointer to a pointer for it to initialise the
 * struct

       unsafe {
           let mut pcamera: _MMAL_COMPONENT_T = mem::uninitialized();
           let status = mmal_component_create(MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_CAMERA, &mut pcamera);
           match status {
               MMAL_STATUS_T::MMAL_SUCCESS => Ok(MMALCamera{ component: Unique::new(pcamera) }),
               e => Err(e),
           }
       }
*/

        unsafe {
            let mut camera_ptr: *mut ffi::MMAL_COMPONENT_T = mem::uninitialized();
            let component: *const c_char =
                ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const c_char;
            let status = ffi::mmal_component_create(component, &mut camera_ptr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(SeriousCamera {
                    camera: Unique::new(&mut *camera_ptr).unwrap(),
                    outputs: Vec::new(),
                    enabled: false,
                    camera_port_enabled: false,
                    pool: mem::zeroed(),
                    still_port_enabled: false,
                    // this is really a hack. ideally these objects wouldn't be structured this way
                    encoder_created: false,
                    encoder_enabled: false,
                    encoder_control_port_enabled: false,
                    encoder_output_port_enabled: false,
                    encoder: mem::zeroed(),
                    connection_created: false,
                    connection: mem::zeroed(),
                    preview_created: false,
                    preview: mem::zeroed(),
                    file: mem::uninitialized(),
                    file_open: false,
                    use_encoder: false,
                }),
                e => Err(e),
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
                s => Err(CameraError::with_status("Unable to set camera number", s)),
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
                    self.encoder = Unique::new(&mut *encoder_ptr).unwrap();
                    self.encoder_created = true;
                    Ok(())
                }
                s => Err(CameraError::with_status("Unable to create encoder", s)),
            }
        }
    }

    pub fn connect_encoder(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut connection_ptr: *mut ffi::MMAL_CONNECTION_T = mem::uninitialized();
            let status = ffi::mmal_connection_create(
                &mut connection_ptr,
                *self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT),
                *self.encoder.as_ref().input.offset(0),
                ffi::MMAL_CONNECTION_FLAG_TUNNELLING | ffi::MMAL_CONNECTION_FLAG_ALLOCATION_ON_INPUT,
            );
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(CameraError::with_status("Unable to create camera->encoder connection", status));
            }

            self.connection = Unique::new(&mut *connection_ptr).unwrap();
            self.connection_created = true;
            let status = ffi::mmal_connection_enable(&mut *connection_ptr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => Err(CameraError::with_status("Unable to enable camera->encoder connection", s))
            }
            // Ok(())
        }
    }

    pub fn enable_control_port(&mut self, get_buffers: bool) -> Result<(), CameraError> {
        unsafe {
            let cb: ffi::MMAL_PORT_BH_CB_T = if get_buffers { Some(camera_buffer_callback) } else { Some(camera_control_callback) };
            let status =
                ffi::mmal_port_enable(self.camera.as_ref().control, cb);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.camera_port_enabled = true;
                    Ok(())
                }
                s => Err(CameraError::with_status("Unable to enable control port", s)),
            }
        }
    }

    pub fn enable_encoder_port(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status =
                ffi::mmal_port_enable(*self.encoder.as_ref().output.offset(0), Some(camera_buffer_callback));
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.encoder_output_port_enabled = true;
                    Ok(())
                }
                s => Err(CameraError::with_status("Unable to enable encoder port", s)),
            }
        }
    }

    pub unsafe fn set_buffer_callback(
        &mut self,
        callback: Box<Fn(Option<&ffi::MMAL_BUFFER_HEADER_T>)>,
    ) {
        let data = Box::new(Userdata {
            pool: self.pool,
            callback: callback,
        });
        let data_ptr = Box::into_raw(data);
        let mut port = if self.use_encoder {
            (*self.encoder.as_ref().output.offset(0))
        } else {
            (*self.camera.as_ref().output.offset(MMAL_CAMERA_CAPTURE_PORT))
        };
        (*port).userdata = data_ptr as *mut ffi::MMAL_PORT_USERDATA_T;
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
            // cfg.one_shot_stills = 1;
            cfg.one_shot_stills = 0;
            cfg.max_preview_video_w = info.max_width;
            cfg.max_preview_video_h = info.max_height;
            cfg.num_preview_video_frames = 1;
            cfg.stills_capture_circular_buffer_height = 0;
            cfg.fast_preview_resume = 0;
            cfg.use_stc_timestamp = ffi::MMAL_PARAMETER_CAMERA_CONFIG_TIMESTAMP_MODE_T::MMAL_PARAM_TIMESTAMP_MODE_RESET_STC;

            let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut cfg.hdr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(()),
                s => Err(CameraError::with_status(
                    "Unable to set control port parmaeter",
                    s,
                )),
            }
        }
    }

    pub fn set_camera_format(
        &mut self,
        mut encoding: c_uint,
        width: u32,
        height: u32,
        zero_copy: bool,
        use_encoder: bool,
    ) -> Result<(), CameraError> {
        unsafe {
            self.use_encoder = use_encoder;

            let output = self.camera.as_ref().output;
            let output_num = self.camera.as_ref().output_num;
            assert_eq!(
                output_num,
                3,
                concat!(
                    "Expected camera to have 3 outputs got: ",
                    stringify!(output_num)
                )
            );

            let preview_port_ptr =
                *(output.offset(MMAL_CAMERA_PREVIEW_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let video_port_ptr =
                *(output.offset(MMAL_CAMERA_VIDEO_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let still_port_ptr =
                *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let preview_port = *preview_port_ptr;
            mem::forget(preview_port);
            let mut video_port = *video_port_ptr;
            mem::forget(video_port);
            let mut still_port = *still_port_ptr;
            mem::forget(still_port);

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
            mem::forget(format);

            if use_encoder {
                (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            } else {
                (*format).encoding = encoding;
                (*format).encoding_variant = 0; //Irrelevant when not in opaque mode
            }
            // (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            let mut es = (*format).es;
            mem::forget(es);

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
                return Err(CameraError::with_status(
                    "Unable to set preview port format",
                    status,
                ));
            }

            if video_port.buffer_num < VIDEO_OUTPUT_BUFFERS_NUM {
                video_port.buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
            }

            // Set the same format on the video port (which we don't use here)
            ffi::mmal_format_full_copy(video_port.format, preview_port.format);
            status = ffi::mmal_port_format_commit(video_port_ptr);

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(CameraError::with_status(
                    "Unable to set video port format",
                    status,
                ));
            }

            format = still_port.format;
            mem::forget(format);

            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L799

            if use_encoder {
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
            mem::forget(es);

            (*es).video.width = ffi::vcos_align_up(width, 32);
            (*es).video.height = ffi::vcos_align_up(height, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = width as i32;
            (*es).video.crop.height = height as i32;
            (*es).video.frame_rate.num = 0; //STILLS_FRAME_RATE_NUM;
            (*es).video.frame_rate.den = 1; //STILLS_FRAME_RATE_DEN;

            // TODO: should this be before or after the commit?
            if still_port.buffer_size < still_port.buffer_size_min {
                still_port.buffer_size = still_port.buffer_size_min;
            }

            still_port.buffer_num = still_port.buffer_num_recommended;

            if zero_copy {
                status = ffi::mmal_port_parameter_set_boolean(
                    video_port_ptr,
                    ffi::MMAL_PARAMETER_ZERO_COPY as u32,
                    ffi::MMAL_TRUE as i32,
                );

                if status != MMAL_STATUS_T::MMAL_SUCCESS {
                    return Err(CameraError::with_status(
                        "Unable to enable zero copy",
                        status,
                    ));
                }
            }

            status = ffi::mmal_port_format_commit(still_port_ptr);
            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(CameraError::with_status(
                    "Unable to set still port format",
                    status,
                ));
            }

            if !use_encoder {
                return Ok(());
            }

            let encoder_in_port_ptr = *(self.encoder.as_ref().input.offset(0) as *mut *mut ffi::MMAL_PORT_T);
            let encoder_out_port_ptr = *(self.encoder.as_ref().output.offset(0) as *mut *mut ffi::MMAL_PORT_T);
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
                return Err(CameraError::with_status(
                    "Unable to set encoder output port format",
                    status,
                ));
            }

            if encoding == ffi::MMAL_ENCODING_JPEG || encoding == ffi::MMAL_ENCODING_MJPEG {
                // Set the JPEG quality level
                status = ffi::mmal_port_parameter_set_uint32(encoder_out_port_ptr, ffi::MMAL_PARAMETER_JPEG_Q_FACTOR, 90);
                if status != MMAL_STATUS_T::MMAL_SUCCESS {
                    return Err(CameraError::with_status(
                        "Unable to set JPEG quality",
                        status,
                    ));
                }

                // Set the JPEG restart interval
                status = ffi::mmal_port_parameter_set_uint32(encoder_out_port_ptr, ffi::MMAL_PARAMETER_JPEG_RESTART_INTERVAL, 0);
                if status != MMAL_STATUS_T::MMAL_SUCCESS {
                    return Err(CameraError::with_status(
                        "Unable to set JPEG restart interval",
                        status,
                    ));
                }
            }

            // TODO: thumbnails
            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStill.c#L1290

            Ok(())
        }
    }

    pub fn enable(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_component_enable(&mut *self.camera.as_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.enabled = true;
                    Ok(())
                }
                s => Err(CameraError::with_status(
                    "Unable to enable camera component",
                    s,
                )),
            }
        }
    }

    pub fn enable_encoder(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_port_enable(self.encoder.as_ref().control, None);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.encoder_control_port_enabled = true;

                    let status = ffi::mmal_component_enable(self.encoder.as_ptr());
                    match status {
                        MMAL_STATUS_T::MMAL_SUCCESS => {
                            self.encoder_enabled = true;
                            Ok(())
                        }
                        s => Err(CameraError::with_status(
                            "Unable to enable encoder component",
                            s,
                        )),
                    }
                }
                s => Err(CameraError::with_status(
                    "Unable to enable encoder control port",
                    s,
                )),
            }
        }
    }

    pub fn enable_preview(&mut self) -> Result<(), CameraError> {
        unsafe {
            let status = ffi::mmal_component_enable(&mut *self.preview.as_ptr());
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    // TODO: fix
                    // self.enabled = true;
                    Ok(())
                }
                s => Err(CameraError::with_status("Unable to enable preview", s)),
            }
        }
    }

    pub fn create_pool(&mut self) -> Result<(), CameraError> {
        unsafe {
            let port_ptr = if self.use_encoder {
                let output = self.encoder.as_ref().output;
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
                Err(CameraError::with_status(
                    concat!(
                        "Failed to create buffer header pool for camera port",
                        stringify!((*port_ptr).name)
                    ),
                    0, // there is no status here unusually
                ))
            } else {
                self.pool = Unique::new(pool).unwrap();
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
                    self.preview = Unique::new(&mut *preview_ptr).unwrap();
                    self.preview_created = true;
                    Ok(())
                }
                s => Err(CameraError::with_status(
                    "Unable to create null sink for preview",
                    s,
                )),
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
            let preview_input_ptr = self.preview.as_ref().input.offset(0);

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
                s => Err(CameraError::with_status(
                    "Unable to connect preview ports",
                    s,
                )),
            }
        }
    }

    pub fn take(&mut self) -> Result<(), CameraError> {
        unsafe {
            let mut status = ffi::mmal_port_parameter_set_uint32(
                self.camera.as_ref().control,
                ffi::MMAL_PARAMETER_SHUTTER_SPEED as u32,
                0, // 0 = auto
            );

            if status != ffi::MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(CameraError::with_status(
                    "Unable to set shutter speed",
                    status,
                ));
            }

            // Send all the buffers to the camera output port
            let num = ffi::mmal_queue_length(self.pool.as_ref().queue as *mut _);
            println!("got length {}", num);
            let output = self.camera.as_ref().output;

            let still_port_ptr =
                *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as *mut *mut ffi::MMAL_PORT_T);
            let buffer_port_ptr;

            if self.use_encoder {
                let encoder_out_port_ptr = *(self.encoder.as_ref().output as *mut *mut ffi::MMAL_PORT_T);
                buffer_port_ptr = encoder_out_port_ptr;
            } else {
                buffer_port_ptr = still_port_ptr;
            }

            println!("assigning pool of {} buffers size {}", (*buffer_port_ptr).buffer_num, (*buffer_port_ptr).buffer_size);

            for i in 0..num {
                let buffer = ffi::mmal_queue_get(self.pool.as_ref().queue);
                println!("got buffer {}", i);

                if buffer.is_null() {
                    return Err(CameraError::with_status(
                        stringify!(format!(
                            "Unable to get a required buffer {} from pool queue",
                            i
                        )),
                        MMAL_STATUS_T::MMAL_STATUS_MAX,
                    ));
                } else {
                    status = ffi::mmal_port_send_buffer(buffer_port_ptr, buffer);
                    if status != MMAL_STATUS_T::MMAL_SUCCESS {
                        return Err(CameraError::with_status(
                            stringify!(format!(
                                "Unable to send a buffer to camera output port ({})",
                                i
                            )),
                            status,
                        ));
                    }
                }
            }

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
                    Ok(())
                }
                s => Err(CameraError::with_status(
                    "Unable to set camera capture boolean",
                    s,
                )),
            }
        }
    }
}

unsafe extern "C" fn camera_buffer_callback(
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    let bytes_to_write = (*buffer).length;
    let mut complete = false;

    println!("I'm called from C. buffer length: {}", bytes_to_write);

    let pdata_ptr: *mut Userdata = (*port).userdata as *mut Userdata;
    let pdata: &mut Userdata = &mut *pdata_ptr;

    if !pdata_ptr.is_null() {
        if bytes_to_write > 0 {
            ffi::mmal_buffer_header_mem_lock(buffer);

            (pdata.callback)(Some(&mut *buffer));

            ffi::mmal_buffer_header_mem_unlock(buffer);
        }

        // Check end of frame or error
        if ((*buffer).flags
            & (ffi::MMAL_BUFFER_HEADER_FLAG_FRAME_END
                | ffi::MMAL_BUFFER_HEADER_FLAG_TRANSMISSION_FAILED)) > 0
        {
            complete = true;
        }
    } else {
        println!("Received a camera still buffer callback with no state");
    }

    // release buffer back to the pool
    ffi::mmal_buffer_header_release(buffer);

    // and send one back to the port (if still open)
    if (*port).is_enabled > 0 && !pdata_ptr.is_null() {
        let mut status: ffi::MMAL_STATUS_T::Type = ffi::MMAL_STATUS_T::MMAL_STATUS_MAX;
        let new_buffer: *mut ffi::MMAL_BUFFER_HEADER_T =
            ffi::mmal_queue_get(pdata.pool.as_ref().queue);

        // and back to the port from there.
        if !new_buffer.is_null() {
            status = ffi::mmal_port_send_buffer(port, new_buffer);
        }

        if new_buffer.is_null() || status != MMAL_STATUS_T::MMAL_SUCCESS {
            println!("Unable to return the buffer to the camera still port");
        }
    }

    if complete {
        println!("complete");
        (pdata.callback)(None);
        Box::from_raw(pdata_ptr); // Allow rust to free this memory
    }

    println!("I'm done with c");
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
            if self.connection_created {
                ffi::mmal_connection_disable(self.connection.as_ptr());
                ffi::mmal_connection_destroy(self.connection.as_ptr());
            }
            if self.encoder_enabled {
                ffi::mmal_component_disable(self.encoder.as_ptr());
                println!("encoder disabled");
            }
            if self.enabled {
                ffi::mmal_component_disable(self.camera.as_ptr());
                println!("camera disabled");
            }
            if self.encoder_control_port_enabled {
                ffi::mmal_port_disable(self.encoder.as_ref().control);
                println!("port disabled");
            }
            if self.camera_port_enabled {
                ffi::mmal_port_disable(self.camera.as_ref().control);
                println!("port disabled");
            }
            ffi::mmal_component_destroy(self.camera.as_ptr());
            println!("camera destroyed");
            if self.encoder_created {
                ffi::mmal_component_destroy(self.encoder.as_ptr());
                println!("encoder destroyed");
            }
        }
    }
}

pub struct SimpleCamera {
    info: CameraInfo,
    serious: SeriousCamera,
}

impl SimpleCamera {
    pub fn new(info: CameraInfo) -> Result<SimpleCamera, u32> {
        let sc = SeriousCamera::new()?;

        Ok(SimpleCamera {
            info: info,
            serious: sc,
        })
    }

    pub fn activate(&mut self) -> Result<(), CameraError> {
        let camera = &mut self.serious;

        camera.set_camera_num(0)?;
        camera.create_encoder()?;
        camera.set_camera_params(&self.info)?;

        camera.create_preview()?;

        // camera.set_camera_format(ffi::MMAL_ENCODING_JPEG, self.info.max_width, self.info.max_height, false)?;
        camera.set_camera_format(
            ffi::MMAL_ENCODING_JPEG,
            self.info.max_width,
            self.info.max_height,
            false,
            true,
        )?;
        camera.enable_control_port(false)?;


        camera.enable()?;
        camera.enable_encoder()?; // only needed if processing image eg returning jpeg
        camera.create_pool()?;

        camera.connect_preview()?;
        // camera.enable_preview()?;

        camera.connect_encoder()?;

        Ok(())
    }

    pub fn take_one(&mut self) -> Result<Vec<u8>, String> {
        let (sender, receiver) = mpsc::sync_channel(1);

        let cb = Box::new(move |o: Option<&ffi::MMAL_BUFFER_HEADER_T>| {
            if o.is_none() {
                sender.send(None).unwrap();
                return;
            }

            let buf = o.unwrap();
            let s = unsafe { slice::from_raw_parts((*buf).data, (*buf).length as usize) };

            sender.send(Some(Box::new(s.to_vec()))).unwrap();
        });

        unsafe {
            self.serious.set_buffer_callback(cb);
        }

        if self.serious.use_encoder {
            if !self.serious.encoder_output_port_enabled {
                self.serious.enable_encoder_port().unwrap();
            }
        } else {
            if !self.serious.still_port_enabled {
                self.serious.enable_still_port().unwrap();
            }
        }

        self.serious
            .take()
            .map_err(|e| format!("take error: {}", e))?;

        let mut c = Vec::new();
        loop {
            let mut b = receiver.recv().map_err(|_| "Could not receive buffer")?;

            if b.is_none() {
                if c.len() == 0 {
                    return Err("Expected to receive some data".into());
                }
                return Ok(c);
            }

            c.extend(*b.unwrap());
        }
    }
}
