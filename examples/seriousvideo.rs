use rascam::*;
use std::fs::File;
use std::io::Write;
use std::sync::mpsc::RecvTimeoutError;
use std::{thread, time};

fn main() {
    let info = info().unwrap();
    if info.cameras.len() < 1 {
        println!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    println!("{}", info);

    serious_video(&info.cameras[0]);
}

fn serious_video(info: &CameraInfo) {
    let mut camera = SeriousCamera::new().unwrap();
    println!("camera created");
    camera.set_camera_num(0).unwrap();
    println!("camera number set");
    camera.enable_control_port(false).unwrap();
    println!("camera control port enabled");

    let settings = CameraSettings {
        encoding: MMAL_ENCODING_H264,
        width: 1920, // 96px will not require padding
        height: 1088,
        iso: ISO_AUTO,
        zero_copy: false,
        use_encoder: true,
        framerate: 30,
        video_profile: MMAL_VIDEO_PROFILE_H264_HIGH,
        video_level: MMAL_VIDEO_LEVEL_H264_4,
    };
    camera
        .set_camera_params(info, false, settings.framerate)
        .unwrap();
    println!("camera params set");
    camera.create_video_encoder().unwrap();
    println!("video encoder created");
    camera.set_video_camera_format(&settings).unwrap();
    println!("set camera format");
    camera.enable().unwrap();
    println!("camera enabled");
    camera.enable_encoder().unwrap();
    println!("encoder enabled");

    camera.create_pool().unwrap();
    println!("pool created");

    // camera.create_preview().unwrap();
    // println!("preview created");
    // camera.connect_preview().unwrap();
    // println!("preview connected");
    // camera.enable_preview().unwrap();
    // println!("preview enabled");

    camera.connect_encoder().unwrap();
    println!("encoder connected");

    // Warm up the camera
    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    println!("recording 5s of video");
    let receiver = camera.take().unwrap();

    // TODO: disabling ports may flush buffers which causes
    // problems if we have already closed the file!

    let mut file = File::create("video.h264").unwrap();
    let time_start = time::Instant::now();
    let sleep_duration = time::Duration::from_millis(5000);

    loop {
        match receiver.recv_timeout(time::Duration::from_millis(500)) {
            Ok(msg) => {
                let buffer = msg.unwrap();
                file.write_all(&buffer.get_bytes()).unwrap();
            }
            Err(RecvTimeoutError::Timeout) => (), // ignore
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let now = time::Instant::now();
        if now.duration_since(time_start) > sleep_duration {
            println!("waited more than sleep time ({:?})", sleep_duration);
            break;
        }
    }

    // If we don't drop `receiver` before `camera`, we get stuck in a deadlock.
    // With no usage of `drop()`, the compiler gives us the desired order.
    drop(receiver);

    println!("dropping camera");
    drop(camera);
    println!("dropped camera");

    println!("Raw h264 bytes written to video.h264");
    println!("Try: vlc video.h264 --demux h264");
}
