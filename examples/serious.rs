use rascam::*;
use std::fs::File;
use std::io::Write;
use std::{thread, time};

fn main() {
    let info = info().unwrap();
    if info.cameras.len() < 1 {
        println!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    println!("{}", info);

    serious(&info.cameras[0]);
}

fn serious(info: &CameraInfo) {
    let mut camera = SeriousCamera::new().unwrap();
    println!("camera created");
    camera.set_camera_num(0).unwrap();
    println!("camera number set");
    camera.create_encoder().unwrap();
    println!("encoder created");
    camera.enable_control_port(true).unwrap();
    println!("camera control port enabled");
    camera.set_camera_params(info, true, 0).unwrap();
    println!("camera params set");

    let settings = CameraSettings {
        encoding: MMAL_ENCODING_RGB24,
        width: 96, // 96px will not require padding
        height: 96,
        iso: ISO_AUTO,
        zero_copy: true,
        use_encoder: false,
        framerate: 0,
        video_level: 0,
        video_profile: 0,
    };

    camera.set_camera_format(&settings).unwrap();
    println!("set camera format");
    camera.enable().unwrap();
    println!("camera enabled");
    camera.create_pool().unwrap();
    println!("pool created");

    camera.create_preview().unwrap();
    println!("preview created");
    camera.connect_preview().unwrap();
    println!("preview connected");
    camera.enable_preview().unwrap();
    println!("preview enabled");

    println!("taking photo");

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    let receiver = camera.take().unwrap();

    let buffer = receiver.recv().unwrap().unwrap();

    File::create("image.rgb")
        .unwrap()
        .write_all(&buffer.get_bytes())
        .unwrap();

    println!("Raw rgb bytes written to image.rgb");
    println!("Try: convert -size 96x96 -depth 8 -colorspace RGB rgb:image.rgb image.png");
    // If imagemagick gives something like:
    //   convert-im6.q16: unexpected end-of-file `image.rgb': No such file or directory @ error/rgb.c/ReadRGBImage/239.
    // There is probably padding in the image. Check the width.
}
