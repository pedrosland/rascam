#[cfg(feature = "test-rpi")]
mod common;

#[cfg(feature = "test-rpi")]
mod integration {

    use super::common;
    use serial_test::serial;

    /// Note that this test will fail under low light conditions
    /// if the camera can't maintain exposure settings etc at 10 fps.
    #[test]
    #[serial]
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
            video_profile: rascam::MMAL_VIDEO_PROFILE_H264_HIGH,
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_4,
            ..Default::default()
        };

        common::record_video(num_secs, settings.clone(), path).unwrap();

        let probe = common::probe_stream(path.into()).unwrap();

        // We can't trust ffprobe's framerate as it doesn't know - it just defaults to 25 fps.
        // This is because our h264 stream doesn't include timing information.
        // A similar check for a MP4 file should have timing information.
        let framerate = probe.num_frames as f32 / num_secs as f32;
        common::assert_framerate(framerate, settings.framerate);
    }

    /// Test that video dimensions for h264 are respected.
    /// h264 needs width to be a multiple of 32 and height a multiple of 16.
    /// The width and height for this test are already multiples of 32 and 16.
    #[test]
    #[serial]
    fn video_friendly_dimensions() {
        let path = "video.h264";
        let num_secs = 1;
        let settings = rascam::CameraSettings {
            // shared
            encoding: rascam::MMAL_ENCODING_H264,
            width: 128,
            height: 112,
            use_encoder: true,
            // video
            framerate: 10,
            video_profile: rascam::MMAL_VIDEO_PROFILE_H264_HIGH,
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_4,
            ..Default::default()
        };

        common::record_video(num_secs, settings.clone(), path).unwrap();

        let probe = common::probe_stream(path.into()).unwrap();

        assert_eq!(probe.width, settings.width);
        assert_eq!(probe.height, settings.height);
    }

    /// Test that video dimensions for h264 are respected.
    /// h264 needs width to be a multiple of 32 and height a multiple of 16.
    /// The width and height for this test are not multiples of 32 and 16 and need cropped.
    #[test]
    #[serial]
    fn video_cropped_dimensions() {
        let path = "video.h264";
        let num_secs = 1;
        let settings = rascam::CameraSettings {
            // shared
            encoding: rascam::MMAL_ENCODING_H264,
            width: 100,
            height: 100,
            use_encoder: true,
            // video
            framerate: 10,
            video_profile: rascam::MMAL_VIDEO_PROFILE_H264_HIGH,
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_4,
            ..Default::default()
        };

        common::record_video(num_secs, settings.clone(), path).unwrap();

        let probe = common::probe_stream(path.into()).unwrap();

        assert_eq!(probe.width, settings.width);
        assert_eq!(probe.height, settings.height);

        assert!(probe.coded_width > settings.width);
        assert!(probe.coded_height > settings.height);
    }

    /// Test that large video dimensions and framerate for h264 are supported without hanging.
    #[test]
    #[serial]
    fn video_large_data() {
        let path = "video.h264";
        let num_secs = 1;
        let settings = rascam::CameraSettings {
            // shared
            encoding: rascam::MMAL_ENCODING_H264,
            width: 1920,
            height: 1080,
            use_encoder: true,
            // video
            framerate: 60,
            video_profile: rascam::MMAL_VIDEO_PROFILE_H264_HIGH,
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_4,
            ..Default::default()
        };

        // This is expected to produce logs from lib-mmal because this fails. Similar to the following:
        //
        // mmal: mmal_vc_port_enable: failed to enable port vc.ril.video_encode:in:0(OPQV): EINVAL
        // mmal: mmal_port_enable: failed to enable connected port (vc.ril.video_encode:in:0(OPQV))0xb672c730 (EINVAL)
        // mmal: mmal_connection_enable: output port couldn't be enabled
        let result = common::record_video(num_secs, settings.clone(), path);
        assert!(
            result.is_err(),
            "expected error trying to encode too much data for the given h264 level"
        );

        let settings = rascam::CameraSettings {
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_42,
            ..settings
        };
        common::record_video(num_secs, settings.clone(), path).unwrap();

        let probe = common::probe_stream(path.into()).unwrap();

        assert_eq!(probe.width, settings.width);
        assert_eq!(probe.height, settings.height);
        // It would be nice to assert on framerate but we can't always capture 1080p@60fps with sensible exposure.
        // When we have more control (shutter speed) we could assert on framerate.
    }

    /// Test that h264 profile and level set.
    #[test]
    #[serial]
    fn h264_profile_and_level() {
        let path = "video.h264";
        let num_secs = 1;
        let settings = rascam::CameraSettings {
            // shared
            encoding: rascam::MMAL_ENCODING_H264,
            width: 100,
            height: 100,
            use_encoder: true,
            // video
            framerate: 10,
            video_profile: rascam::MMAL_VIDEO_PROFILE_H264_HIGH,
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_4,
            ..Default::default()
        };

        common::record_video(num_secs, settings.clone(), path).unwrap();

        let probe = common::probe_stream(path.into()).unwrap();

        assert_eq!(probe.profile, "High");
        assert_eq!(probe.level, 40);

        // Change the settings

        let settings = rascam::CameraSettings {
            video_profile: rascam::MMAL_VIDEO_PROFILE_H264_BASELINE,
            video_level: rascam::MMAL_VIDEO_LEVEL_H264_42,
            ..settings
        };

        common::record_video(num_secs, settings.clone(), path).unwrap();

        let probe = common::probe_stream(path.into()).unwrap();

        assert_eq!(probe.profile, "Baseline");
        assert_eq!(probe.level, 42);
    }
}
