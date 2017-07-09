#![feature(unique)]
extern crate mmal_sys as ffi;
extern crate libc;
use libc::{c_int, uint32_t, uint16_t, uint8_t, int32_t, size_t, c_char, c_void};
use std::ffi::CStr;
use std::mem;
use std::ptr::Unique;
use std::default::Default;
use std::ptr;

fn main() {
    do_nothing();

    let mut info = CameraInfo::info().unwrap();
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
        println!("  {} {}x{}", ::std::str::from_utf8(&camera.camera_name).unwrap(), camera.max_width, camera.max_height);
    }

  let mut camera = SimpleCamera::new().unwrap();
  println!("camera created");
  camera.set_camera_num(0);
  println!("camera number set");
  camera.create_encoder().unwrap();
  println!("encoder created");
  camera.enable_port().unwrap();
  println!("camera port enabled");
  camera.set_camera_params(info.cameras[0]).unwrap();
  println!("camera params set");
  camera.enable().unwrap();
  println!("camera enabled");
  // camera.connect().unwrap();
  // println!("camera connected");

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
struct CameraInfo {

}

impl CameraInfo {
    pub fn info() -> Result<ffi::MMAL_PARAMETER_CAMERA_INFO_T, ffi::MMAL_STATUS_T> {
        unsafe {
            let mut raw = Box::new(mem::zeroed());
            let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_CAMERA_INFO.as_ptr() as *const ::std::os::raw::c_char;
            let mut raw3: *mut ffi::MMAL_COMPONENT_T = &mut *raw;
            // println!("component: {:#?}\ncamera_info: {:#?}", *component, *raw3);
            let status = ffi::mmal_component_create(component, &mut raw3 as *mut _);
            // println!("2 status: {:#?}\ncomponent: {:#?}\ncamera_info: {:#?}", status, *component, *raw3);

            match status {
                ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                    let mut found = false;
                    let mut param = Box::new(mem::zeroed());
                    let mut param3: *mut ffi::MMAL_PARAMETER_CAMERA_INFO_T = &mut *param;
                    (*param3).hdr.id = ffi::MMAL_PARAMETER_CAMERA_INFO as u32;
                    (*param3).hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_INFO_T>() as u32;

                    // println!("3 camera info status {:?}\nparam: {:#?}\ncontrol: {:#?}", status, (*param3).hdr, (*(*raw3).control).priv_);
                    let status = ffi::mmal_port_parameter_get((*raw3).control, &mut (*param3).hdr);
                    found = status == ffi::MMAL_STATUS_T::MMAL_SUCCESS;

                    ffi::mmal_component_destroy(raw3);

                    if !found {
                        Err(status)
                    } else {
                        Ok(*param3)
                    }
                },
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

    encoder: Unique<ffi::MMAL_COMPONENT_T>,
    encoder_created: bool,
    encoder_enabled: bool,
    encoder_port_enabled: bool,

    connection: Unique<ffi::MMAL_CONNECTION_T>,
    connection_created: bool,
}

impl SimpleCamera {
    pub fn new() -> Result<SimpleCamera, ffi::MMAL_STATUS_T> {
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
           let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const ::std::os::raw::c_char;
           let mut camera_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *camera;
           let status = ffi::mmal_component_create(component, &mut camera_ptr as *mut _);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => Ok(SimpleCamera{
                   camera: Unique::new(&mut *camera_ptr),
                   outputs: Vec::new(),
                   enabled: false,
                   port_enabled: false,
                   // this is really a hack. ideally these objects wouldn't be structured this way
                   encoder_created: false,
                   encoder_enabled: false,
                   encoder_port_enabled: false,
                   encoder: mem::zeroed(),
                   connection_created: false,
                   connection: mem::zeroed(),
               }),
               e => Err(e),
           }
       }
   }

   pub fn set_camera_num(&mut self, num: u8) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
           let mut param = Box::new(mem::zeroed());
           let mut param3: *mut ffi::MMAL_PARAMETER_INT32_T = &mut *param;
           (*param3).hdr.id = ffi::MMAL_PARAMETER_CAMERA_NUM as u32;
           (*param3).hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_INT32_T>() as u32;
           (*param3).value = num as i32;

           let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut (*param3).hdr);
           println!("status {:?}", status);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => Ok(num),
               e => Err(e),
           }
       }
   }

   pub fn create_encoder(&mut self) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
           let mut encoder = Box::new(mem::zeroed());
           let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const ::std::os::raw::c_char;
           let mut encoder_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *encoder;
           let status = ffi::mmal_component_create(component, &mut encoder_ptr as *mut _);
           println!("status {:?}", status);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                   self.encoder = Unique::new(&mut *encoder_ptr);
                   self.encoder_created = true;
                   Ok(1)
               },
               e => Err(e),
           }
       }
   }

   pub fn connect(&mut self) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
           let mut connection: ffi::MMAL_CONNECTION_T = mem::zeroed();
           let mut state_ptr = &mut connection as *mut _;
           let status = ffi::mmal_connection_create(&mut state_ptr as *mut _, *self.camera.as_ref().output, *self.encoder.as_ref().input, 0);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                   self.connection = Unique::new(&mut connection);
                   self.connection_created = true;
                   let status = ffi::mmal_connection_enable(self.connection.as_ptr());
                   match status {
                       ffi::MMAL_STATUS_T::MMAL_SUCCESS => Ok(1),
                       e => Err(e)
                   }
               },
               e => Err(e),
           }
       }
   }

   pub fn enable_port(&mut self) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
        //    let mut encoder = Box::new(self.camera.as_ref());
        //    let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const ::std::os::raw::c_char;
        //    let mut encoder_ptr: *mut ffi::MMAL_COMPONENT_T = &mut *encoder;
        //    let status = ffi::mmal_component_create(component, &mut encoder_ptr as *mut _);

           let status = ffi::mmal_port_enable(self.camera.as_ref().control, Some(port_callback));
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                   self.port_enabled = true;
                   Ok(1)
               },
               e => Err(e)
           }
       }
   }

   pub fn set_camera_params(&mut self, info: ffi::MMAL_PARAMETER_CAMERA_INFO_CAMERA_T) -> Result<u8, ffi::MMAL_STATUS_T> {
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

           let status = ffi::mmal_port_parameter_set(self.camera.as_ref().control, &mut (*param3).hdr);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => Ok(1),
               e => Err(status)
           }
       }
   }

   pub fn enable(&mut self) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
           let status = ffi::mmal_component_enable(&mut *self.camera.as_ptr() as *mut _);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                   self.enabled = true;
           //
        //            let status = ffi::mmal_port_enable(self.encoder.as_ref().control, None);
        //            match status {
        //                ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
        //                    self.encoder_port_enabled = true;
           //
        //                   let status = ffi::mmal_component_enable(self.encoder.as_ptr());
        //                   match status {
        //                       ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
        //                           self.encoder_enabled = true;
                                  Ok(1)
        //                       },
        //                       e => Err(e),
        //                   }
        //               },
        //               e => Err(e),
                //   }
               },
               e => Err(e)
           }
       }
   }
}

#[no_mangle]
unsafe extern "C" fn port_callback(port: *mut ffi::MMAL_PORT_T, buffer: *mut ffi::MMAL_BUFFER_HEADER_T) {
    println!("I'm called from C");
    // unsafe {
    //     // Update the value in RustObject with the value received from the callback:
    //     (*target).a = a;
    // }
}

impl Drop for SimpleCamera
{
  fn drop(&mut self)
  {
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
