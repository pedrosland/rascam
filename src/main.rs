#![feature(unique)]
extern crate mmal_sys as ffi;
extern crate libc;
extern crate bytes;
extern crate cv;
use ffi::MMAL_STATUS_T;
use libc::{c_int, uint32_t, uint16_t, uint8_t, int32_t, size_t, c_char, c_void};
use std::ffi::CStr;
use std::mem;
use std::ptr::Unique;
use std::fs::File;
use std::io::prelude::*;
use std::slice;
use std::{thread, time};
use cv::*;

const MMAL_CAMERA_PREVIEW_PORT: isize = 0;
const MMAL_CAMERA_VIDEO_PORT: isize = 1;
const MMAL_CAMERA_CAPTURE_PORT: isize = 2;

/// Video render needs at least 2 buffers.
const VIDEO_OUTPUT_BUFFERS_NUM: u32 = 3;

const PREVIEW_FRAME_RATE_NUM: i32 = 0;
const PREVIEW_FRAME_RATE_DEN: i32 = 1;

fn main() {
    do_nothing();

    let info = CameraInfo::info().unwrap();
    // println!("camera info {:?}", info);
    if info.num_cameras < 1 {
        println!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    println!("Found {} camera(s)", info.num_cameras);

    // We can't iterate over all cameras because we will always have 4.
    // Alternatively, we could iterate and break early. Not sure if that is more rust-y
    for index in 0..info.num_cameras {
        let camera = info.cameras[index as usize];
        println!(
            "  {} {}x{}",
            ::std::str::from_utf8(&camera.camera_name).unwrap(),
            camera.max_width,
            camera.max_height
        );
    }

    let mut camera = SimpleCamera::new().unwrap();
    println!("camera created");
    camera.set_camera_num(0).unwrap();
    println!("camera number set");
    camera.create_encoder().unwrap();
    println!("encoder created");
    camera.enable_control_port().unwrap();
    println!("camera control port enabled");
    camera.set_camera_params(info.cameras[0]).unwrap();
    println!("camera params set");

    /*
   // Ensure there are enough buffers to avoid dropping frames
   if (video_port->buffer_num < VIDEO_OUTPUT_BUFFERS_NUM)
   video_port->buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
  */
    camera.set_camera_format(info.cameras[0]).unwrap();
    println!("set camera format");
    camera.enable().unwrap();
    println!("camera enabled");
    camera.create_pool().unwrap();
    println!("pool created");

    camera.create_preview().unwrap();
    println!("preview created");
    camera.enable_preview().unwrap();
    println!("preview enabled");

    // camera.connect().unwrap();
    camera.connect_ports().unwrap();
    println!("camera ports connected");

    camera.enable_still_port().unwrap();
    println!("camera still port enabled");


    camera.take().unwrap();
    println!("taking photo");

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    // https://github.com/thaytan/gst-rpicamsrc/blob/master/src/RaspiCapture.c#L259

    // src->capture_config.width = info.width;
    // src->capture_config.height = info.height;
    // src->capture_config.fps_n = info.fps_n;
    // src->capture_config.fps_d = info.fps_d;


    // src->capture_config.encoding = MMAL_ENCODING_JPEG;
    // src->capture_config.encoding = MMAL_ENCODING_BGR24
}

/**
 * do_nothing exists purely to ensure that the libraries we need are actually linked.
 * Something does some optimization (perhaps --as-needed) and doesn't link unused libraries.
 * Unfortunatly, without these, we get the following error:
 *
 * mmal: mmal_component_create_core: could not find component 'vc.ril.camera'
 *
 * See this for more info https://github.com/thaytan/gst-rpicamsrc/issues/28
 */
fn do_nothing() {
    unsafe {
        ffi::bcm_host_init();
        ffi::vcos_init();
        ffi::mmal_vc_init();
    }
}

#[repr(C)]
#[derive(Debug)]
struct CameraInfo {}

impl CameraInfo {
    pub fn info() -> Result<ffi::MMAL_PARAMETER_CAMERA_INFO_T, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let mut raw = Box::new(mem::zeroed());
            let component: *const ::std::os::raw::c_char =
                ffi::MMAL_COMPONENT_DEFAULT_CAMERA_INFO.as_ptr() as *const ::std::os::raw::c_char;
            let mut raw3: *mut ffi::MMAL_COMPONENT_T = &mut *raw;
            // println!("component: {:#?}\ncamera_info: {:#?}", *component, *raw3);
            let status = ffi::mmal_component_create(component, &mut raw3 as *mut _);
            // println!("2 status: {:#?}\ncomponent: {:#?}\ncamera_info: {:#?}", status, *component, *raw3);

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    let mut found = false;
                    let mut param = Box::new(mem::zeroed());
                    let mut param3: *mut ffi::MMAL_PARAMETER_CAMERA_INFO_T = &mut *param;
                    (*param3).hdr.id = ffi::MMAL_PARAMETER_CAMERA_INFO as u32;
                    (*param3).hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_INFO_T>() as u32;

                    // println!("3 camera info status {:?}\nparam: {:#?}\ncontrol: {:#?}", status, (*param3).hdr, (*(*raw3).control).priv_);
                    let status = ffi::mmal_port_parameter_get((*raw3).control, &mut (*param3).hdr);
                    found = status == MMAL_STATUS_T::MMAL_SUCCESS;

                    ffi::mmal_component_destroy(raw3);

                    if !found {
                        Err(status)
                    } else {
                        Ok(*param3)
                    }
                }
                e => Err(e),
            }
        }
    }
}

#[repr(C)]
struct SimpleCamera {
    camera: Unique<ffi::MMAL_COMPONENT_T>,
    outputs: Vec<ffi::MMAL_PORT_T>,
    enabled: bool,
    port_enabled: bool,
    still_port_enabled: bool,
    pool: Unique<ffi::MMAL_POOL_T>,

    encoder: Unique<ffi::MMAL_COMPONENT_T>,
    encoder_created: bool,
    encoder_enabled: bool,
    encoder_port_enabled: bool,

    connection: Unique<ffi::MMAL_CONNECTION_T>,
    connection_created: bool,

    preview: Unique<ffi::MMAL_COMPONENT_T>,
    preview_created: bool,

    file: File,
    file_open: bool,
}

impl SimpleCamera {
    pub fn new() -> Result<SimpleCamera, ffi::MMAL_STATUS_T::Type> {
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
            let mut camera = Box::new(mem::zeroed());
            let component: *const ::std::os::raw::c_char =
                ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const ::std::os::raw::c_char;
            let mut camera_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *camera;
            let status = ffi::mmal_component_create(component, &mut camera_ptr as *mut _);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(SimpleCamera {
                    camera: Unique::new(&mut *camera_ptr).unwrap(),
                    outputs: Vec::new(),
                    enabled: false,
                    port_enabled: false,
                    pool: mem::zeroed(),
                    still_port_enabled: false,
                    // this is really a hack. ideally these objects wouldn't be structured this way
                    encoder_created: false,
                    encoder_enabled: false,
                    encoder_port_enabled: false,
                    encoder: mem::zeroed(),
                    connection_created: false,
                    connection: mem::zeroed(),
                    preview_created: false,
                    preview: mem::zeroed(),
                    file: mem::uninitialized(),
                    file_open: false,
                    // zero_copy_cb:
                }),
                e => Err(e),
            }
        }
    }

    pub fn set_camera_num(&mut self, num: u8) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let mut param = Box::new(mem::zeroed());
            let mut param3: *mut ffi::MMAL_PARAMETER_INT32_T = &mut *param;
            (*param3).hdr.id = ffi::MMAL_PARAMETER_CAMERA_NUM as u32;
            (*param3).hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_INT32_T>() as u32;
            (*param3).value = num as i32;

            let status =
                ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut (*param3).hdr);
            println!("status {:?}", status);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => Ok(num),
                e => Err(e),
            }
        }
    }

    pub fn create_encoder(&mut self) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let mut encoder = Box::new(mem::zeroed());
            let component: *const ::std::os::raw::c_char =
                ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const ::std::os::raw::c_char;
            let mut encoder_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *encoder;
            let status = ffi::mmal_component_create(component, &mut encoder_ptr as *mut _);
            println!("status {:?}", status);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.encoder = Unique::new(&mut *encoder_ptr).unwrap();
                    self.encoder_created = true;
                    Ok(1)
                }
                e => Err(e),
            }
        }
    }

    pub fn connect(&mut self) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let mut connection: ffi::MMAL_CONNECTION_T = mem::zeroed();
            let mut state_ptr = &mut connection as *mut _;
            let status = ffi::mmal_connection_create(
                &mut state_ptr as *mut _,
                *self.camera.as_ref().output,
                *self.encoder.as_ref().input,
                0,
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.connection = Unique::new(&mut connection).unwrap();
                    self.connection_created = true;
                    let status = ffi::mmal_connection_enable(self.connection.as_ptr());
                    match status {
                        MMAL_STATUS_T::MMAL_SUCCESS => Ok(1),
                        e => Err(e),
                    }
                }
                e => Err(e),
            }
        }
    }

    pub fn enable_control_port(&mut self) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            //    let mut encoder = Box::new(self.camera.as_ref());
            //    let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const ::std::os::raw::c_char;
            //    let mut encoder_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *encoder;
            //    let status = ffi::mmal_component_create(component, &mut encoder_ptr as *mut _);

            let status =
                ffi::mmal_port_enable(self.camera.as_ref().control, Some(camera_control_callback));
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.port_enabled = true;
                    Ok(1)
                }
                e => Err(e),
            }
        }
    }

    pub fn enable_still_port(&mut self) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let self_ptr = self as *mut SimpleCamera;
            (**self.camera.as_ref().output.offset(2)).userdata = self_ptr as
                *mut ffi::MMAL_PORT_USERDATA_T;

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

    pub fn set_camera_params(
        &mut self,
        info: ffi::MMAL_PARAMETER_CAMERA_INFO_CAMERA_T,
    ) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let mut param = Box::new(mem::zeroed());
            let mut param3: *mut ffi::MMAL_PARAMETER_CAMERA_CONFIG_T = &mut *param;
            (*param3).hdr.id = ffi::MMAL_PARAMETER_CAMERA_CONFIG as u32;
            (*param3).hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_CONFIG_T>() as u32;

            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L706
            (*param3).max_stills_w = info.max_width;
            (*param3).max_stills_h = info.max_height;
            (*param3).stills_yuv422 = 0;
            (*param3).one_shot_stills = 1;
            (*param3).max_preview_video_w = info.max_width;
            (*param3).max_preview_video_h = info.max_height;
            (*param3).num_preview_video_frames = 1;
            (*param3).stills_capture_circular_buffer_height = 0;
            (*param3).fast_preview_resume = 0;
            (*param3).use_stc_timestamp = ffi::MMAL_PARAMETER_CAMERA_CONFIG_TIMESTAMP_MODE_T::MMAL_PARAM_TIMESTAMP_MODE_RESET_STC;

            mem::forget((*param3).max_stills_w);
            mem::forget(param);
            mem::forget(param3);
            let status =
                ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut (*param3).hdr);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    let mut param = Box::new(mem::zeroed());
                    let mut newParam3: *mut ffi::MMAL_PARAMETER_CAMERA_CONFIG_T = &mut *param;
                    (*newParam3).hdr.id = ffi::MMAL_PARAMETER_CAMERA_CONFIG as u32;
                    (*newParam3).hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_CONFIG_T>() as u32;

                    let status = ffi::mmal_port_parameter_get(self.camera.as_ref().control, &mut (*newParam3).hdr);
                    println!("3 camera info {:?}\nparam: {:#?}\nnew param3: {:#?}\nparam3: {:#?}", status, (*newParam3).hdr, (*newParam3), (*param3));


                    Ok(1)
                },
                e => Err(status),
            }
        }
    }

    pub fn set_camera_format(
        &mut self,
        info: ffi::MMAL_PARAMETER_CAMERA_INFO_CAMERA_T,
    ) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
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

            // the following lines are not very pretty or safe
            let preview_port_ptr = *(output.offset(MMAL_CAMERA_PREVIEW_PORT) as
                                             *mut *mut ffi::MMAL_PORT_T);
            let video_port_ptr = *(output.offset(MMAL_CAMERA_VIDEO_PORT) as
                                           *mut *mut ffi::MMAL_PORT_T);
            let still_port_ptr = *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as
                                           *mut *mut ffi::MMAL_PORT_T);
            let preview_port = *preview_port_ptr;
            mem::forget(preview_port);
            let mut video_port = *video_port_ptr;
            mem::forget(video_port);
            let mut still_port = *still_port_ptr;
            mem::forget(still_port);

            // TODO:
            //raspicamcontrol_set_all_parameters(camera, &state->camera_parameters);

            let mut format = preview_port.format;
            mem::forget(format);

            // (*format).encoding = ffi::MMAL_ENCODING_BGR24;
            (*format).encoding = ffi::MMAL_ENCODING_RGB24;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;
            // (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            let mut es = (*format).es;
            mem::forget(es);

            //   Use a full FOV 4:3 mode
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
                return Err(status);
            }

            // Set the same format on the video  port (which we don't use here)
            ffi::mmal_format_full_copy(video_port.format, preview_port.format);
            status = ffi::mmal_port_format_commit(video_port_ptr);

            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                return Err(status);
            }

            if video_port.buffer_num < VIDEO_OUTPUT_BUFFERS_NUM {
                video_port.buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
            }

            format = still_port.format;
            mem::forget(format);

            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L799

            /*
             * On firmware prior to June 2016, camera and video_splitter
             * had BGR24 and RGB24 support reversed.
             */
            (*format).encoding = if ffi::mmal_util_rgb_order_fixed(still_port_ptr) == 1 {
                ffi::MMAL_ENCODING_RGB24
            } else {
                ffi::MMAL_ENCODING_BGR24
            };
            (*format).encoding_variant = 0; //Irrelevant when not in opaque mode

            // (*still_port.format).encoding = ffi::MMAL_ENCODING_JPEG;
            // (*still_port.format).encoding_variant = ffi::MMAL_ENCODING_JPEG;

            // (*format).encoding = ffi::MMAL_ENCODING_I420;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;
            // (*format).encoding = ffi::MMAL_ENCODING_OPAQUE;
            // (*format).encoding_variant = ffi::MMAL_ENCODING_I420;

            // es = elementary stream
            es = (*format).es;
            mem::forget(es);

            (*es).video.width = ffi::vcos_align_up(info.max_width, 32);
            (*es).video.height = ffi::vcos_align_up(info.max_height, 16);
            (*es).video.crop.x = 0;
            (*es).video.crop.y = 0;
            (*es).video.crop.width = info.max_width as i32;
            (*es).video.crop.height = info.max_height as i32;
            (*es).video.frame_rate.num = 0; //STILLS_FRAME_RATE_NUM;
            (*es).video.frame_rate.den = 1; //STILLS_FRAME_RATE_DEN;

            if still_port.buffer_size < still_port.buffer_size_min {
                still_port.buffer_size = still_port.buffer_size_min;
            }

            still_port.buffer_num = still_port.buffer_num_recommended;

            status = ffi::mmal_port_parameter_set_boolean(
                video_port_ptr,
                ffi::MMAL_PARAMETER_ZERO_COPY as u32,
                ffi::MMAL_TRUE as i32,
            );

            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    println!("set_camera_format still_port: {:?}, \nformat: {:?}\nes: {:?}\nencoding I420: {:?}\nencoding opaque: {:?}\nencoding jpeg: {:?}\nencoding rgb24: {:?}", still_port, *still_port.format, (*(*still_port.format).es).video, ffi::MMAL_ENCODING_I420, ffi::MMAL_ENCODING_OPAQUE, ffi::MMAL_ENCODING_JPEG, ffi::MMAL_ENCODING_RGB24);
                    let status = ffi::mmal_port_format_commit(still_port_ptr);
                    match status {
                        MMAL_STATUS_T::MMAL_SUCCESS => Ok(1),
                        e => Err(status),
                    }
                }
                e => {
                    println!("Failed to select zero copy");
                    Err(status)
                }
            }
        }
    }

    pub fn enable(&mut self) -> Result<u8, ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let status = ffi::mmal_component_enable(&mut *self.camera.as_ptr() as *mut _);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.enabled = true;
                    //
                    //            let status = ffi::mmal_port_enable(self.encoder.as_ref().control, None);
                    //            match status {
                    //                MMAL_STATUS_T::MMAL_SUCCESS => {
                    //                    self.encoder_port_enabled = true;
                    //
                    //                   let status = ffi::mmal_component_enable(self.encoder.as_ptr());
                    //                   match status {
                    //                       MMAL_STATUS_T::MMAL_SUCCESS => {
                    //                           self.encoder_enabled = true;
                    Ok(1)
                    //                       },
                    //                       e => Err(e),
                    //                   }
                    //               },
                    //               e => Err(e),
                    //   }
                }
                e => Err(e),
            }
        }
    }

    pub fn enable_preview(&mut self) -> Result<(), ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let status = ffi::mmal_component_enable(&mut *self.preview.as_ptr() as *mut _);
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    // TODO: fix
                    // self.enabled = true;
                    Ok(())
                }
                e => Err(e),
            }
        }
    }

    pub fn create_pool(&mut self) -> Result<(), String> {
        unsafe {
            let output = self.camera.as_ref().output;
            let still_port_ptr = *(output.offset(2) as *mut *mut ffi::MMAL_PORT_T);
            let pool = ffi::mmal_port_pool_create(
                still_port_ptr,
                (*still_port_ptr).buffer_num,
                (*still_port_ptr).buffer_size,
            );

            if pool.is_null() {
                println!(
                    "Failed to create buffer header pool for camera still port {:?}",
                    (*still_port_ptr).name
                );
                Err("Null pool".to_string())
            } else {
                self.pool = Unique::new(pool).unwrap();
                Ok(())
            }
        }
    }

    pub fn create_preview(&mut self) -> Result<(), ffi::MMAL_STATUS_T::Type> {
        unsafe {
            // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiPreview.c#L70
            // https://github.com/waveform80/picamera/issues/22
            // and the commit message that closed issue #22
            let mut preview = Box::new(mem::zeroed());
            let mut preview_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *preview;
            // Note that there appears to be no constant for the null sink but it does exist in the
            // binaries.
            let status = ffi::mmal_component_create(
                b"vc.null_sink\x00".as_ptr(),
                &mut preview_ptr as *mut _,
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    self.preview = Unique::new(&mut *preview_ptr).unwrap();
                    self.preview_created = true;
                    Ok(())
                }
                e => Err(e),
            }
        }
    }

    pub fn connect_ports(&mut self) -> Result<(), ffi::MMAL_STATUS_T::Type> {
        unsafe {
            let mut connection = Box::new(mem::zeroed());
            let mut connection_ptr: *mut ffi::MMAL_CONNECTION_T = &mut *connection;

            let preview_output_ptr = self.camera
                .as_ref()
                .output
                .offset(MMAL_CAMERA_PREVIEW_PORT as isize);
            let preview_input_ptr = self.preview.as_ref().input.offset(0);

            let status = ffi::mmal_connection_create(
                &mut connection_ptr as *mut _,
                *preview_output_ptr,
                *preview_input_ptr,
                ffi::MMAL_CONNECTION_FLAG_TUNNELLING |
                    ffi::MMAL_CONNECTION_FLAG_ALLOCATION_ON_INPUT,
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {
                    // self.preview = Unique::new(&mut *preview_ptr);
                    // self.preview_created = true;
                    Ok(())
                }
                e => Err(e),
            }
        }
    }

    // TODO: callback when complete? future? stream?
    pub fn take(&mut self) -> Result<(), ffi::MMAL_STATUS_T::Type> {
        let sleep_duration = time::Duration::from_millis(5000);
        thread::sleep(sleep_duration);

        unsafe {
            // speed 0 = auto
            let mut status = ffi::mmal_port_parameter_set_uint32(
                self.camera.as_ref().control,
                ffi::MMAL_PARAMETER_SHUTTER_SPEED as u32,
                0,
            );
            match status {
                MMAL_STATUS_T::MMAL_SUCCESS => {

                    // Send all the buffers to the camera output port
                    let num = ffi::mmal_queue_length(self.pool.as_ref().queue as *mut _);
                    println!("got length {}", num);
                    let output = self.camera.as_ref().output;
                    let still_port_ptr = *(output.offset(MMAL_CAMERA_CAPTURE_PORT) as
                                                   *mut *mut ffi::MMAL_PORT_T);
                    let i: u8 = 0;

                    for i in 0..num {
                        let buffer = ffi::mmal_queue_get(self.pool.as_ref().queue);
                        println!("got buffer {}", i);

                        if buffer.is_null() {
                            println!("Unable to get a required buffer {} from pool queue", i);
                            status = MMAL_STATUS_T::MMAL_STATUS_MAX;
                            break;
                        } else {
                            status = ffi::mmal_port_send_buffer(still_port_ptr, buffer);
                            if status != MMAL_STATUS_T::MMAL_SUCCESS {
                                break;
                            }
                        }
                    }

                    match status {
                        MMAL_STATUS_T::MMAL_SUCCESS => {
                            status = ffi::mmal_port_parameter_set_boolean(
                                still_port_ptr,
                                ffi::MMAL_PARAMETER_CAPTURE as u32,
                                1,
                            );

                            match status {
                                MMAL_STATUS_T::MMAL_SUCCESS => {
                                    println!("Started capture");
                                    Ok(())
                                },
                                e => {
                                    println!("Could not set camera capture boolean");
                                    Err(e)
                                }
                            }
                        }
                        e => {
                            // TODO: is this the same "i" that is being used in the loop?
                            println!("Unable to send a buffer to camera output port ({})", i);
                            Err(e)
                        }
                    }
                }
                e => {
                    println!("unable to set shutter speed");
                    Err(e)
                }
            }
        }
    }
}

#[no_mangle]
unsafe extern "C" fn camera_buffer_callback(
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    // unsafe {
    //     // Update the value in RustObject with the value received from the callback:
    //     (*target).a = a;
    // }
    //

    // let mut bytes_written: i32 = 0;
    let bytes_to_write = (*buffer).length;

    println!("I'm called from C. buffer length: {}", bytes_to_write);

    // TODO: this is probably unsafe as Rust has no knowledge of this so it could have already been
    // dropped!
    let pdata_ptr: *mut SimpleCamera = (*port).userdata as *mut SimpleCamera;
    let mut pdata: &mut SimpleCamera = &mut *pdata_ptr;

    if !pdata_ptr.is_null() {

        if !pdata.file_open {
            pdata.file_open = true;
            pdata.file = File::create("image1.rgb").unwrap();
            // pdata.file2 = File::create("image2.rgb").unwrap();
        }

        if bytes_to_write > 0
        // && pdata->file_handle
        {
            ffi::mmal_buffer_header_mem_lock(buffer);

            let s = slice::from_raw_parts(
                (*buffer).data,
                bytes_to_write as usize,
            );

            pdata
                .file
                .write_all(&s)
                .expect("Saving raw buffer image failed");
            // bytes_written = fwrite(buffer.data, 1, bytes_to_write, pdata->file_handle);

            // let img = image::ImageBuffer::from_raw(2592, 1944, slice::from_raw_parts((*buffer).data, bytes_to_write as usize)).expect("Opening image failed");
            // let mut out = File::create("image.jpg").unwrap();
            // img.save("image.jpg").expect("Saving image failed");

            // OpenCV try #1:
            // let mat = Mat::imdecode(&s, cv::imgcodecs::ImreadModes::ImreadColor);
            // mat.cvt_color(cv::imgproc::ColorConversionCodes::BGR2RGB);
            //
            // let s2 = mat.imencode("png", vec!()).expect("Could not create PNG");

            // OpenCV try #2:
            // TODO: fix this
            // let size = (3280 * 2463 * 3) as usize;
            // println!("calculated size: {}, got size: {}", size, bytes_to_write);
            // let mut vec = Vec::with_capacity(bytes_to_write as usize);
            //
            // vec.set_len(bytes_to_write as usize);
            // std::ptr::copy((*buffer).data, vec.as_mut_ptr(), bytes_to_write as usize);
            //
            // // It would be nice if something like this existed in the rust bindings:
            // // https://docs.opencv.org/3.4.0/d3/d63/classcv_1_1Mat.html#a51615ebf17a64c968df0bf49b4de6a3a
            // let new_image = Mat::from_buffer(3280, 2463, CvType::Cv8UC3 as i32, &vec);
            // // new_image.cvt_color(cv::imgproc::ColorConversionCodes::BGR2RGB);
            // new_image.cvt_color(cv::imgproc::ColorConversionCodes::YUV2BGR_IYUV);
            //
            // // There's also imwrite()
            // let s2 = new_image.imencode(".png", vec!()).expect("Could not create PNG");
            //
            // pdata.file2
            //      .write_all(&s2)
            //      .expect("Saving png raw buffer image failed");
            //
            // mem::forget(vec);

            // THIS WORKS:
            // let size = (3280 * 2464 * 3) as usize;
            // let mut s2 = Vec::with_capacity(bytes_to_write as usize);
            // let mem_width = ffi::vcos_align_up(3280, 32) as usize;
            // let data_length = 3280 * 3;
            //
            // for i in 0..2464 {
            //     // s2[i*3280*3..(i+1)*3280*3].copy_from_slice(&s[i*mem_width*3..(i+1)*mem_width*3]);
            //     let row_offset = i*mem_width*3;
            //     s2.extend(&s[row_offset..row_offset+data_length]);
            // }
            //
            // pdata.file2
            //      .write_all(&s2)
            //      .expect("Saving modified raw buffer image failed");

            ffi::mmal_buffer_header_mem_unlock(buffer);
        }

        // Check end of frame or error
        if ((*buffer).flags &
                (ffi::MMAL_BUFFER_HEADER_FLAG_FRAME_END |
                     ffi::MMAL_BUFFER_HEADER_FLAG_TRANSMISSION_FAILED)) > 0
        {
            // complete = true;
            println!("complete");
            // drop(pdata.file);
        }
    } else {
        println!("Received a camera still buffer callback with no state");
    }

    // release buffer back to the pool
    ffi::mmal_buffer_header_release(buffer);

    // and send one back to the port (if still open)
    if (*port).is_enabled > 0 {
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

    //    if (complete)
    //    {
    //       vcos_semaphore_post(&(pData->complete_semaphore));
    // }
    println!("I'm done with c");
}

#[no_mangle]
unsafe extern "C" fn camera_control_callback(
    port: *mut ffi::MMAL_PORT_T,
    buffer: *mut ffi::MMAL_BUFFER_HEADER_T,
) {
    // https://github.com/raspberrypi/userland/blob/master/host_applications/linux/apps/raspicam/RaspiStillYUV.c#L525

    println!("Camera control callback  cmd=0x{:08x}", (*buffer).cmd);

    if (*buffer).cmd == ffi::MMAL_EVENT_PARAMETER_CHANGED {
        let param: *mut ffi::MMAL_EVENT_PARAMETER_CHANGED_T = (*buffer).data as
            *mut ffi::MMAL_EVENT_PARAMETER_CHANGED_T;
        if (*param).hdr.id == (ffi::MMAL_PARAMETER_CAMERA_SETTINGS as u32) {
            let settings_ptr: *mut ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T = param as
                *mut ffi::MMAL_PARAMETER_CAMERA_SETTINGS_T;
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

impl Drop for SimpleCamera {
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
            if self.port_enabled {
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
