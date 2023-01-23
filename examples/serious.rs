use rascam::*;
use std::fs::File;
use std::io::Write;
use std::{thread, time};
use tracing::{error, info};

fn main() {
    // Set up logging to stdout
    tracing_subscriber::fmt::init();

    let info = info().unwrap();
    if info.cameras.len() < 1 {
        error!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    info!("{}", info);

    serious(&info.cameras[0]);
}

fn serious(info: &CameraInfo) {
    let mut camera = SeriousCamera::new().unwrap();
    info!("camera created");
    camera.set_camera_num(0).unwrap();
    info!("camera number set");
    camera.create_encoder().unwrap();
    info!("encoder created");
    camera.enable_control_port(true).unwrap();
    info!("camera control port enabled");
    camera.set_camera_params(info).unwrap();
    info!("camera params set");

    let settings = CameraSettings {
        encoding: MMAL_ENCODING_RGB24,
        width: 96, // 96px will not require padding
        height: 96,
        iso: ISO_AUTO,
        zero_copy: true,
        use_encoder: false,
    };

    camera.set_camera_format(&settings).unwrap();
    info!("set camera format");
    camera.enable().unwrap();
    info!("camera enabled");
    camera.create_pool().unwrap();
    info!("pool created");

    camera.create_preview().unwrap();
    info!("preview created");
    camera.connect_preview().unwrap();
    info!("preview connected");
    camera.enable_preview().unwrap();
    info!("preview enabled");

    info!("taking photo");

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    let receiver = camera.take().unwrap();

    let buffer = receiver.recv().unwrap().unwrap();

    File::create("image.rgb")
        .unwrap()
        .write_all(&buffer.get_bytes())
        .unwrap();

    info!("Raw rgb bytes written to image.rgb");
    info!("Try: convert -size 96x96 -depth 8 -colorspace RGB rgb:image.rgb image.png");
    // If imagemagick gives something like:
    //   convert-im6.q16: unexpected end-of-file `image.rgb': No such file or directory @ error/rgb.c/ReadRGBImage/239.
    // There is probably padding in the image. Check the width.
}
