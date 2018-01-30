//use cv::*;
// use futures::Future;
extern crate cam;

use std::fs::File;
use std::io::Write;
use std::{thread, time};
use cam::*;

fn main() {
    let info = info().unwrap();
    // println!("camera info {:?}", info);
    if info.cameras.len() < 1 {
        println!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    println!("{}", info);

    if true {
        simple(&info.cameras[0]);
    } else {
        serious(&info.cameras[0]);
    }
}

fn simple(info: &CameraInfo) {
    let mut camera = SimpleCamera::new(info.clone()).unwrap();
    camera.activate().unwrap();

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    let b = camera.take_one().unwrap();
    File::create("image1.rgb").unwrap().write_all(&b).unwrap();
}

fn serious(info: &CameraInfo) {
    let mut camera = SeriousCamera::new().unwrap();
    println!("camera created");
    camera.set_camera_num(0).unwrap();
    println!("camera number set");
    camera.create_encoder().unwrap();
    println!("encoder created");
    camera.enable_control_port().unwrap();
    println!("camera control port enabled");
    camera.set_camera_params(info).unwrap();
    println!("camera params set");

    /*
   // Ensure there are enough buffers to avoid dropping frames
   if (video_port->buffer_num < VIDEO_OUTPUT_BUFFERS_NUM)
   video_port->buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
  */
    camera.set_camera_format(info).unwrap();
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

    // camera.enable_still_port(cb).unwrap();
    println!("camera still port enabled");

    // camera.take().unwrap();
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
