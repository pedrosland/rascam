#![feature(unique)]

extern crate libc;
use libc::{c_int, uint32_t, uint16_t, uint8_t, int32_t, size_t, c_char, c_void};
use std::mem;
use std::ptr::Unique;
use std::default::Default;


#[repr(C)]
struct MMAL_COMPONENT_PRIVATE_T;
#[repr(C)]
struct MMAL_COMPONENT_USERDATA_T; //TODO: is this right?

#[repr(C)]
#[derive(PartialEq)]
enum MMAL_STATUS_T {
   MMAL_SUCCESS = 0,                 /**< Success */
   MMAL_ENOMEM,                      /**< Out of memory */
   MMAL_ENOSPC,                      /**< Out of resources (other than memory) */
   MMAL_EINVAL,                      /**< Argument is invalid */
   MMAL_ENOSYS,                      /**< Function not implemented */
   MMAL_ENOENT,                      /**< No such file or directory */
   MMAL_ENXIO,                       /**< No such device or address */
   MMAL_EIO,                         /**< I/O error */
   MMAL_ESPIPE,                      /**< Illegal seek */
   MMAL_ECORRUPT,                    /**< Data is corrupt \attention FIXME: not POSIX */
   MMAL_ENOTREADY,                   /**< Component is not ready \attention FIXME: not POSIX */
   MMAL_ECONFIG,                     /**< Component is not configured \attention FIXME: not POSIX */
   MMAL_EISCONN,                     /**< Port is already connected */
   MMAL_ENOTCONN,                    /**< Port is disconnected */
   MMAL_EAGAIN,                      /**< Resource temporarily unavailable. Try again later*/
   MMAL_EFAULT,                      /**< Bad address */
   /* Do not add new codes here unless they match something from POSIX */
   MMAL_STATUS_MAX = 0x7FFFFFFF,      /*< Force to 32 bit */
}

/** Four Character Code type */
type MMAL_FOURCC_T = uint32_t;

/** Describes a rectangle */
#[repr(C)]
struct MMAL_RECT_T{
    x: int32_t,      /**< x coordinate (from left) */
    y: int32_t,      /**< y coordinate (from top) */
    width: int32_t,  /**< width */
    height: int32_t, /*< height */

}



/** Describes a rational number */
#[repr(C)]
struct MMAL_RATIONAL_T {
    num: int32_t,    /**< Numerator */
    den: int32_t,    /*< Denominator */
}



/** Enumeration of the different types of elementary streams.
 * This divides elementary streams into 4 big categories, plus an invalid type. */
 #[repr(C)]
enum MMAL_ES_TYPE_T {
   MMAL_ES_TYPE_UNKNOWN,     /**< Unknown elementary stream type */
   MMAL_ES_TYPE_CONTROL,     /**< Elementary stream of control commands */
   MMAL_ES_TYPE_AUDIO,       /**< Audio elementary stream */
   MMAL_ES_TYPE_VIDEO,       /**< Video elementary stream */
   MMAL_ES_TYPE_SUBPICTURE,   /*< Sub-picture elementary stream (e.g. subtitles, overlays) */
}


/** Definition of a video format.
 * This describes the properties specific to a video stream */
#[repr(C)]
struct MMAL_VIDEO_FORMAT_T {
   width: uint32_t,        /**< Width of frame in pixels */
   height: uint32_t,       /**< Height of frame in rows of pixels */
        crop: MMAL_RECT_T,         /**< Visible region of the frame */
    frame_rate: MMAL_RATIONAL_T,   /**< Frame rate */
    par: MMAL_RATIONAL_T,          /**< Pixel aspect ratio */

      color_space: MMAL_FOURCC_T,  /*< FourCC specifying the color space of the
                                   * video stream. See the \ref MmalColorSpace
                                   * "pre-defined color spaces" for some examples.
                                   */
}

/** Definition of an audio format.
 * This describes the properties specific to an audio stream */
#[repr(C)]
struct MMAL_AUDIO_FORMAT_T {
    channels: uint32_t,           /**< Number of audio channels */
    sample_rate: uint32_t,        /**< Sample rate */

    bits_per_sample: uint32_t,    /**< Bits per sample */
    block_align: uint32_t,        /*< Size of a block of data */

   /* \todo add channel mapping, gapless and replay-gain support */
}

/** Definition of a subpicture format.
 * This describes the properties specific to a subpicture stream */
#[repr(C)]
struct MMAL_SUBPICTURE_FORMAT_T {
    x_offset: uint32_t,        /**< Width offset to the start of the subpicture */
    y_offset: uint32_t,        /*< Height offset to the start of the subpicture */

   /* \todo surely more things are needed here */

}

/** Definition of the type specific format.
 * This describes the type specific information of the elementary stream. */
#[repr(C)]
struct MMAL_ES_SPECIFIC_FORMAT_T {
   audio: MMAL_AUDIO_FORMAT_T,      /**< Audio specific information */
   video: MMAL_VIDEO_FORMAT_T,      /**< Video specific information */
   subpicture: MMAL_SUBPICTURE_FORMAT_T, /*< Subpicture specific information */
}

/** Definition of an elementary stream format */
#[repr(C)]
struct MMAL_ES_FORMAT_T {
    /** Note that this field is called `type` in C */
    format_type: MMAL_ES_TYPE_T,           /**< Type of the elementary stream */

    encoding: MMAL_FOURCC_T,        /**< FourCC specifying the encoding of the elementary stream.
                                    * See the \ref MmalEncodings "pre-defined encodings" for some
                                    * examples.
                                    */
    encoding_variant: MMAL_FOURCC_T,/**< FourCC specifying the specific encoding variant of
                                    * the elementary stream. See the \ref MmalEncodingVariants
                                    * "pre-defined encoding variants" for some examples.
                                    */

   es: *mut MMAL_ES_SPECIFIC_FORMAT_T, /**< Type specific information for the elementary stream */

    bitrate: uint32_t,              /**< Bitrate in bits per second */
    flags: uint32_t,                /**< Flags describing properties of the elementary stream.
                                    * See \ref elementarystreamflags "Elementary stream flags".
                                    */

    extradata_size: uint32_t,       /**< Size of the codec specific data */
   extradata: *mut uint8_t,           /*< Codec specific data */

}

#[repr(C)]
struct MMAL_PORT_PRIVATE_T;
#[repr(C)]
struct MMAL_PORT_USERDATA_T;

/** List of port types */
#[repr(C)]
enum MMAL_PORT_TYPE_T {
   MMAL_PORT_TYPE_UNKNOWN = 0,          /**< Unknown port type */
   MMAL_PORT_TYPE_CONTROL,              /**< Control port */
   MMAL_PORT_TYPE_INPUT,                /**< Input port */
   MMAL_PORT_TYPE_OUTPUT,               /**< Output port */
   MMAL_PORT_TYPE_CLOCK,                /**< Clock port */
   MMAL_PORT_TYPE_INVALID = 0xffffffff,  /*< Dummy value to force 32bit enum */
}


/** Definition of a port.
 * A port is the entity that is exposed by components to receive or transmit
 * buffer headers (\ref MMAL_BUFFER_HEADER_T). A port is defined by its
 * \ref MMAL_ES_FORMAT_T.
 *
 * It may be possible to override the buffer requirements of a port by using
 * the MMAL_PARAMETER_BUFFER_REQUIREMENTS parameter.
 */
#[repr(C)]
struct MMAL_PORT_T {
    /** Note that this field is called `priv` in C */
   private: *mut MMAL_PORT_PRIVATE_T, /**< Private member used by the framework */
   name: *const c_char,                 /**< Port name. Used for debugging purposes (Read Only) */

   /** Note that this field is called `type` in C */
   port_type: MMAL_PORT_TYPE_T,            /**< Type of the port (Read Only) */
   index: uint16_t,                   /**< Index of the port in its type list (Read Only) */
   index_all: uint16_t,               /**< Index of the port in the list of all ports (Read Only) */

   is_enabled: uint32_t,              /**< Indicates whether the port is enabled or not (Read Only) */
   format: *mut MMAL_ES_FORMAT_T,         /**< Format of the elementary stream */

   buffer_num_min: uint32_t,          /**< Minimum number of buffers the port requires (Read Only).
                                          This is set by the component. */
   buffer_size_min: uint32_t,         /**< Minimum size of buffers the port requires (Read Only).
                                          This is set by the component. */
   buffer_alignment_min: uint32_t,    /**< Minimum alignment requirement for the buffers (Read Only).
                                          A value of zero means no special alignment requirements.
                                          This is set by the component. */
   buffer_num_recommended: uint32_t,  /**< Number of buffers the port recommends for optimal performance (Read Only).
                                          A value of zero means no special recommendation.
                                          This is set by the component. */
   buffer_size_recommended: uint32_t, /**< Size of buffers the port recommends for optimal performance (Read Only).
                                          A value of zero means no special recommendation.
                                          This is set by the component. */
   buffer_num: uint32_t,              /**< Actual number of buffers the port will use.
                                          This is set by the client. */
   buffer_size: uint32_t,             /**< Actual maximum size of the buffers that will be sent
                                          to the port. This is set by the client. */

   component: *mut MMAL_COMPONENT_T,    /**< Component this port belongs to (Read Only) */
   userdata: *mut MMAL_PORT_USERDATA_T, /**< Field reserved for use by the client */

   capabilities: uint32_t,            /*< Flags describing the capabilities of a port (Read Only).
                                       * Bitwise combination of \ref portcapabilities "Port capabilities"
                                       * values.
                                       */
}

#[repr(C)]
struct MMAL_COMPONENT_T {
   /** Pointer to the private data of the module in use */
   /** Note that this is called `priv` in C */
   private: *mut MMAL_COMPONENT_PRIVATE_T,

   /** Pointer to private data of the client */
   userdata: *mut MMAL_COMPONENT_USERDATA_T,

   /** Component name */
   name: *const c_char,

   /** Specifies whether the component is enabled or not */
   is_enabled: uint32_t,

   /** All components expose a control port.
    * The control port is used by clients to set / get parameters that are global to the
    * component. It is also used to receive events, which again are global to the component.
    * To be able to receive events, the client needs to enable and register a callback on the
    * control port. */
   control: *mut MMAL_PORT_T,

   input_num: uint32_t,   /**< Number of input ports */
   input: *mut MMAL_PORT_T,     /**< Array of input ports */

   output_num: uint32_t,  /**< Number of output ports */
   output: *mut MMAL_PORT_T,    /**< Array of output ports */

   clock_num: uint32_t,   /**< Number of clock ports */
   clock: *mut MMAL_PORT_T,     /**< Array of clock ports */

   port_num: uint32_t,    /**< Total number of ports */
   port: *mut MMAL_PORT_T,      /**< Array of all the ports (control/input/output/clock) */

   /** Uniquely identifies the component's instance within the MMAL
    * context / process. For debugging. */
   id: uint32_t,
}

impl Default for MMAL_COMPONENT_T {
  fn default() -> MMAL_COMPONENT_T {
    let pname: *const u8 = "\0".as_ptr();

    unsafe {
      MMAL_COMPONENT_T {
        private: mem::uninitialized(),
        userdata: mem::uninitialized(),
        name: pname,
        is_enabled: 0,
        control: mem::uninitialized(),
        input_num: 0,
        input: mem::uninitialized(),
        output_num: 0,
        output: mem::uninitialized(),
        clock_num: 0,
        clock: mem::uninitialized(),
        port_num: 0,
        port: mem::uninitialized(),
        id: 0,
      }
    }
  }
}



// util/mmal_default_components.h #############################################
#[repr(C)]
enum MMAL_COMPONENT {
    MMAL_COMPONENT_DEFAULT_VIDEO_DECODER,
    MMAL_COMPONENT_DEFAULT_VIDEO_ENCODER,
    MMAL_COMPONENT_DEFAULT_VIDEO_RENDERER,
    MMAL_COMPONENT_DEFAULT_IMAGE_DECODER,
    MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER,
    MMAL_COMPONENT_DEFAULT_CAMERA,
    MMAL_COMPONENT_DEFAULT_VIDEO_CONVERTER,
    MMAL_COMPONENT_DEFAULT_SPLITTER,
    MMAL_COMPONENT_DEFAULT_SCHEDULER,
    MMAL_COMPONENT_DEFAULT_VIDEO_INJECTER,
    MMAL_COMPONENT_DEFAULT_VIDEO_SPLITTER,
    MMAL_COMPONENT_DEFAULT_AUDIO_DECODER,
    MMAL_COMPONENT_DEFAULT_AUDIO_RENDERER,
    MMAL_COMPONENT_DEFAULT_MIRACAST,
    MMAL_COMPONENT_DEFAULT_CLOCK,
    MMAL_COMPONENT_DEFAULT_CAMERA_INFO,
    // The following two components aren't in the MMAL headers, but do exist
    MMAL_COMPONENT_DEFAULT_NULL_SINK,
    MMAL_COMPONENT_DEFAULT_RESIZER,
    MMAL_COMPONENT_DEFAULT_ISP,
    MMAL_COMPONENT_RAW_CAMERA,
}

impl MMAL_COMPONENT {
    pub fn as_str(&self) -> &str {
        match self {
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_VIDEO_DECODER   => "vc.ril.video_decode",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_VIDEO_ENCODER   => "vc.ril.video_encode",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_VIDEO_RENDERER  => "vc.ril.video_render",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_IMAGE_DECODER   => "vc.ril.image_decode",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_IMAGE_ENCODER   => "vc.ril.image_encode",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_CAMERA          => "vc.ril.camera",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_VIDEO_CONVERTER => "vc.video_convert",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_SPLITTER        => "vc.splitter",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_SCHEDULER       => "vc.scheduler",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_VIDEO_INJECTER  => "vc.video_inject",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_VIDEO_SPLITTER  => "vc.ril.video_splitter",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_AUDIO_DECODER   => "none",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_AUDIO_RENDERER  => "vc.ril.audio_render",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_MIRACAST        => "vc.miracast",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_CLOCK           => "vc.clock",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_CAMERA_INFO     => "vc.camera_info",
            // The following two components aren't in the MMAL headers, but do exist
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_NULL_SINK       => "vc.null_sink",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_RESIZER         => "vc.ril.resize",
            &MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_ISP             => "vc.ril.isp",
            &MMAL_COMPONENT::MMAL_COMPONENT_RAW_CAMERA              => "vc.ril.rawcam",
        }
    }
}

type _MMAL_COMPONENT_T = *mut c_void;

#[link(name = "mmal_core")]
#[link(name = "mmal_util")]
extern "C" {
  fn mmal_component_create(component: MMAL_COMPONENT, camera: *mut MMAL_COMPONENT_T) -> MMAL_STATUS_T;
}


// static _camera: &'static *mut MMAL_COMPONENT_T = &MMAL_COMPONENT_T{};

fn main() {
    let camera = MMALCamera::new();

  let use_video_port = false;
  let splitter_port = 0;
  // let (camera_port, output_port) = _get_ports(use_video_port, splitter_port);
}

struct MMALCamera {
    component: Unique<MMAL_COMPONENT_T>,
}

impl MMALCamera {
    pub fn new() -> Result<MMALCamera, MMAL_STATUS_T> {
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
           let mut pcamera: MMAL_COMPONENT_T = Default::default();
           let status = mmal_component_create(MMAL_COMPONENT::MMAL_COMPONENT_DEFAULT_CAMERA, &mut pcamera);
           match status {
               MMAL_STATUS_T::MMAL_SUCCESS => Ok(MMALCamera{ component: Unique::new(&mut pcamera) }),
               e => Err(e),
           }
       }
    }
}

// impl Drop for MMALCamera
// {
//   fn drop(&mut self)
//   {
//     unsafe { ffi::foo_destroy(*self.foo); }
//   }
// }

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
