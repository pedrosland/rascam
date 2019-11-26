use mmal_sys as ffi;
use std::sync::Once;

/// This function must be called before any mmal work. Failure to do so will cause errors like:
///
/// mmal: mmal_component_create_core: could not find component 'vc.camera_info'
///
/// See this for more info https://github.com/thaytan/gst-rpicamsrc/issues/28
pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        ffi::bcm_host_init();
        ffi::vcos_init();
        ffi::mmal_vc_init();
    });
}
