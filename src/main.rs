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

    bench_jpegs_per_sec(5);
    // simple_sync(&info.cameras[0]);
    // serious(&info.cameras[0]);
}

fn simple_sync(info: &CameraInfo) {
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
    camera.enable_control_port(true).unwrap();
    println!("camera control port enabled");
    camera.set_camera_params(info).unwrap();
    println!("camera params set");

    /*
   // Ensure there are enough buffers to avoid dropping frames
   if (video_port->buffer_num < VIDEO_OUTPUT_BUFFERS_NUM)
   video_port->buffer_num = VIDEO_OUTPUT_BUFFERS_NUM;
  */
    camera
        .set_camera_format(
            MMAL_ENCODING_RGB24,
            info.max_width,
            info.max_height,
            true,
            false,
        )
        .unwrap();
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

    // camera.connect_encoder().unwrap();

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

// Benchmarking from https://github.com/seenaburns/raytracer/blob/master/src/bench.rs

// Run function and return result with seconds duration
pub fn time<F, T>(f: F) -> (T, f64)
where
    F: FnOnce() -> T,
{
    let start = time::Instant::now();
    let res = f();
    let end = time::Instant::now();

    let runtime_nanos = end.duration_since(start).subsec_nanos();
    let runtime_secs = runtime_nanos as f64 / 1_000_000_000.0;
    (res, runtime_secs)
}

// Prints iteration execution time and average
pub fn bench_jpegs_per_sec(n: i32) {
    let mut runs: Vec<f64> = Vec::with_capacity(n as usize);

    let info = info().unwrap();
    let mut camera = SimpleCamera::new(info.cameras[0].clone()).unwrap();
    camera.activate().unwrap();

    let mut b = Box::new(camera);

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    for _ in 0..n {
        let images = 20;
        let (_, runtime) = time(|| bench_jpegs(images, &mut b));
        let images_per_sec = images as f64 / runtime;
        println!(
            "{} images in {} sec, {:.2} images/sec",
            images, runtime, images_per_sec
        );
        runs.push(images_per_sec);
    }
    println!(
        "Avg: {:.2} images/sec from {} runs",
        runs.iter().sum::<f64>() / (runs.len() as f64),
        runs.len()
    );
}

pub fn bench_jpegs(n: i32, camera: &mut Box<SimpleCamera>) {
    for _ in 0..n {
        camera.take_one().unwrap();
    }
}
