#![feature(unique)]
extern crate mmal_sys as ffi;
extern crate libc;
use libc::{c_int, uint32_t, uint16_t, uint8_t, int32_t, size_t, c_char, c_void};
use std::mem;
use std::ptr::Unique;
use std::default::Default;


// static _camera: &'static *mut MMAL_COMPONENT_T = &MMAL_COMPONENT_T{};

fn main() {
    let camera = MMALCamera::new();

  let use_video_port = false;
  let splitter_port = 0;
  // let (camera_port, output_port) = _get_ports(use_video_port, splitter_port);
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

struct MMALCamera {
    component: Unique<ffi::MMAL_COMPONENT_T>,
//    control:
}

impl MMALCamera {
    pub fn new() -> Result<MMALCamera, ffi::MMAL_STATUS_T> {
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
           let mut pcamera: ffi::MMAL_COMPONENT_T = mem::zeroed();
           let mut component: *const ::std::os::raw::c_char = ffi::MMAL_COMPONENT_DEFAULT_CAMERA.as_ptr() as *const ::std::os::raw::c_char;
           let mut state_ptr = &mut pcamera as *mut _;
           let status = ffi::mmal_component_create(component, &mut state_ptr as *mut _);
           match status {
               ffi::MMAL_STATUS_T::MMAL_SUCCESS => Ok(MMALCamera{ component: Unique::new(&mut pcamera) }),
               e => Err(e),
           }
       }
    }

    /*

            self._control = MMALControlPort(self._component[0].control)
            port_class = {
                mmal.MMAL_ES_TYPE_UNKNOWN:    MMALPort,
                mmal.MMAL_ES_TYPE_CONTROL:    MMALControlPort,
                mmal.MMAL_ES_TYPE_VIDEO:      MMALVideoPort,
                mmal.MMAL_ES_TYPE_AUDIO:      MMALAudioPort,
                mmal.MMAL_ES_TYPE_SUBPICTURE: MMALSubPicturePort,
                }
            self._inputs = tuple(
                port_class[self._component[0].input[n][0].format[0].type](
                    self._component[0].input[n], opaque_subformat)
                for n, opaque_subformat in enumerate(self.opaque_input_subformats))
            self._outputs = tuple(
                port_class[self._component[0].output[n][0].format[0].type](
                    self._component[0].output[n], opaque_subformat)
    for n, opaque_subformat in enumerate(self.opaque_output_subformats))
     */
}

impl Drop for MMALCamera
{
  fn drop(&mut self)
  {
    unsafe { ffi::mmal_component_destroy(self.component.as_ptr()); }
  }
}

// fn set_ports(from_video_port: bool, splitter_port: u32) -> (?, ?) {
// /*
// self._check_camera_open()
// if from_video_port and (splitter_port in self._encoders):
//     raise PiCameraAlreadyRecording(
//             'The camera is already using port %d ' % splitter_port)
// camera_port = (
//     self._camera.outputs[self.CAMERA_VIDEO_PORT]
//     if from_video_port else
//     self._camera.outputs[self.CAMERA_CAPTURE_PORT]
//     )
// output_port = (
//     self._splitter.outputs[splitter_port]
//     if from_video_port else
//     camera_port
//     )
// return (camera_port, output_port)
// */
//
//     let camera_port = if from_video_port {
//         _camera.outputs[self.CAMERA_VIDEO_PORT];
//     } else {
//         _camera.outputs[self.CAMERA_CAPTURE_PORT];
//     };
//
//     let output_port = if from_video_port {
//         _splitter.outputs[splitter_port];
//     } else {
//         camera_port;
//     };
//
//     return (camera_port, output_port);
// }


/*
// https://github.com/waveform80/picamera/blob/974540f38793c79c69405948e2fffafb3a1025dd/picamera/camera.py#L1663

def capture_sequence(
    outputs, format='jpeg', use_video_port=False, resize=None,
    splitter_port=0, burst=False, bayer=False, **options)

camera_port, output_port = self._get_ports(use_video_port, splitter_port)
format = self._get_image_format(output, format)
encoder = self._get_image_encoder(
    camera_port, output_port, format, resize, **options)


while True:
    if bayer:
        camera_port.params[mmal.MMAL_PARAMETER_ENABLE_RAW_CAPTURE] = True
    encoder.start(output)
    if not encoder.wait(self.CAPTURE_TIMEOUT):
        raise PiCameraRuntimeError(
            'Timed out waiting for capture to end')
    yield output

finally:
    encoder.close()
 */
