use rascam::*;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::{thread, time};

fn main() {
    let info = info().unwrap();
    if info.cameras.len() < 1 {
        println!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    println!("{}", info);

    simple_video(&info.cameras[0]);
}

fn simple_video(info: &CameraInfo) {
    let mut camera = SimpleCamera::new(info.clone()).unwrap();
    camera.configure(CameraSettings {
        // shared
        encoding: MMAL_ENCODING_H264,
        width: info.max_width / 2,
        height: info.max_height / 2,
        use_encoder: true,
        // video
        framerate: 2,
        video_profile: MMAL_VIDEO_PROFILE_H264_HIGH,
        video_level: MMAL_VIDEO_LEVEL_H264_4,
        ..Default::default()
    });
    camera.activate().unwrap();

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    println!("Recording video for 5s");

    let mut file = BufWriter::new(File::create("video.h264").unwrap());
    let frame_iter = camera.take_video_frame_writer().unwrap();

    let handle = thread::spawn(move || {
        for frame in frame_iter {
            file.write_all(&frame[..]).unwrap();
        }
        file
    });

    let sleep_duration = time::Duration::from_millis(5000);
    thread::sleep(sleep_duration);

    camera.stop();

    {
        let mut file = handle.join().unwrap();
        file.flush().unwrap();
    }

    println!("Saved video as video.h264");
}
