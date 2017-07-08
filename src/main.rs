#![feature(unique)]
extern crate mmal_sys as ffi;
extern crate libc;
use libc::{c_int, uint32_t, uint16_t, uint8_t, int32_t, size_t, c_char, c_void};
use std::mem;
use std::ptr::Unique;
use std::default::Default;
use std::ptr;

fn main() {
    do_nothing();

    let mut info = SimpleCamera::info().unwrap();
    println!("camera info {:?}", info);

  // let mut camera = SimpleCamera::new().unwrap();
  // println!("camera created");
  // camera.create_encoder().unwrap();
  // println!("encoder created");
  // // camera.enable().unwrap();
  // // println!("camera enabled");
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
    pub fn info() -> Result<ffi::MMAL_PARAMETER_CAMERA_INFO_T, ffi::MMAL_STATUS_T> {
        unsafe {
            let mut camera_info1: ffi::MMAL_COMPONENT_T = std::mem::uninitialized();
            let mut camera_info: *mut ffi::MMAL_COMPONENT_T = &mut camera_info1;
            // let mut p: ffi::MMAL_COMPONENT_PRIVATE_T = mem::zeroed();
            // camera_info.priv_ = p;
            let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_CAMERA_INFO.as_ptr() as *const ::std::os::raw::c_char;
            println!("component: {:#?}\ncamera_info: {:#?}", *component, (*camera_info).control);
            // println!("camera info\ncontrol: {:#?}", (*camera_info1.control));
            let status = ffi::mmal_component_create(component, &mut camera_info);
            println!("2 component: {:#?}\ncamera_info: {:#?}", *component, (*camera_info).control);
            // println!("camera info\ncontrol: {:#?}", (*camera_info1.control));
            match status {
                ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                    let mut found = false;
                    // try smallest structure to largest as later firmwares reject
                    // older structures
                    // for s in info_structs{
                        let mut param1: ffi::MMAL_PARAMETER_CAMERA_INFO_T = std::mem::uninitialized();
                        let mut param: *mut ffi::MMAL_PARAMETER_CAMERA_INFO_T = &mut param1;
                        param1.hdr.id = ffi::MMAL_PARAMETER_CAMERA_INFO as u32;
                        param1.hdr.size = mem::size_of::<ffi::MMAL_PARAMETER_CAMERA_INFO_T>() as u32;

                        // let mut priv_: ffi::MMAL_PORT_PRIVATE_T = std::mem::zeroed();
                        // (*camera_info1.control).priv_ = &mut priv_;
                        println!("camera info status {:?}, param: {:#?}\ncontrol: {:#?}", status, param1.hdr, (*camera_info1.control).priv_);
                        let status = ffi::mmal_port_parameter_get(camera_info1.control, &mut param1.hdr);
                        found = true;
                            // break;
                    // }

                    ffi::mmal_component_destroy(camera_info);

                    if !found {
                        Err(status)
                    } else {
                        Ok(*param)
                    }
                },
                e => Err(e),
            }
        }
    }

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
           let mut camera: ffi::MMAL_COMPONENT_T = mem::zeroed();
           let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const ::std::os::raw::c_char;
           let mut camera_ptr = &mut camera as *mut _;
           let status = ffi::mmal_component_create(component, &mut camera_ptr as *mut _);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => Ok(SimpleCamera{
                   camera: Unique::new(&mut camera),
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

   pub fn create_encoder(&mut self) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
           let mut encoder: ffi::MMAL_COMPONENT_T = mem::zeroed();
           let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER.as_ptr() as *const ::std::os::raw::c_char;
           let mut encoder_ptr = &mut encoder as *mut _;
           let status = ffi::mmal_component_create(component, &mut encoder_ptr as *mut _);
           println!("status {:?}", status);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                   self.encoder = Unique::new(&mut encoder);
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

   pub fn enable(&mut self) -> Result<u8, ffi::MMAL_STATUS_T> {
       unsafe {
           let status = ffi::mmal_port_enable(self.camera.as_ref().control, Some(port_callback));
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                   self.port_enabled = true;

                //    let status = ffi::mmal_component_enable(self.camera.as_ptr());
                //    match status {
                //        ffi::MMAL_STATUS_T::MMAL_SUCCESS => {
                //            self.enabled = true;
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
                //           }
                //        },
                //        e => Err(e),
                //    }
               },
               e => Err(e),
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
        // if self.connection_created {
        //     ffi::mmal_connection_disable(self.connection.as_ptr());
        //     ffi::mmal_connection_destroy(self.connection.as_ptr());
        // }
        // if self.encoder_enabled {
        //     ffi::mmal_component_disable(self.encoder.as_ptr());
        //     println!("encoder disabled");
        // }
        // if self.enabled {
        //     ffi::mmal_component_disable(self.camera.as_ptr());
        //     println!("camera disabled");
        // }
        // if self.port_enabled {
        //     ffi::mmal_port_disable(self.camera.as_ref().control);
        // }
        ffi::mmal_component_destroy(self.camera.as_ptr());
        println!("camera destroyed");
        if self.encoder_created {
            ffi::mmal_component_destroy(self.encoder.as_ptr());
            println!("encoder destroyed");
        }
    }
  }
}
