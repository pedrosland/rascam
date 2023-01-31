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

    simple_sync(&info.cameras[0]);
}

fn simple_sync(info: &CameraInfo) {
    let mut camera = SimpleCamera::new(info.clone()).unwrap();
    camera.activate().unwrap();

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    let b = camera.take_one().unwrap();
    File::create("image.jpg").unwrap().write_all(&b).unwrap();

    info!("Saved image as image.jpg");
}
