#[cfg(feature = "test-rpi")]
mod common;

#[cfg(feature = "test-rpi")]
mod integration {

    use super::common;

    /// Note that this test will fail under low light conditions
    /// if the camera can't maintain exposure settings etc at 10 fps.
    #[test]
    fn video_framerate_10fps() {
        let path = "video.h264";
        let num_secs = 5;
        let settings = rascam::CameraSettings {
            // shared
            encoding: rascam::MMAL_ENCODING_H264,
            width: 1920,
            height: 1088,
            use_encoder: true,
            // video
            framerate: 10,
            video_profile: rascam::MMAL_VIDEO_LEVEL_H264_4,
            video_level: rascam::MMAL_VIDEO_PROFILE_H264_HIGH,
            ..Default::default()
        };

        common::record_video(num_secs, settings, path);

        let probe = common::probe_stream(path.into()).unwrap();

        // We can't trust ffprobe's framerate as it doesn't know - it just defaults to 25 fps.
        // This is because our h264 stream doesn't include timing information.
        // A similar check for a MP4 file should have timing information.
        let framerate = probe.num_frames as f32 / num_secs as f32;
        common::assert_framerate(framerate, 10);
    }

}
