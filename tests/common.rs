#![cfg(test)]
#![cfg(feature = "test-rpi")]

use serde;
use serde::de::{self, Visitor};
use serde::Deserialize;
use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::iter;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time;

use rascam::CameraSettings;

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ProbeInfo {
    pub(crate) streams: Vec<ProbeStream>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ProbeStream {
    pub(crate) codec_name: String,
    /// Valid values include: Baseline
    pub(crate) profile: String,
    /// This can be negative.
    ///
    /// Numbers do not correspond with the constants used in mmal but the h264 values eg 42 = MMAL_VIDEO_LEVEL_H264_42.
    pub(crate) level: i32,
    pub(crate) codec_type: String,
    #[serde(deserialize_with = "deserialize_hex")]
    pub(crate) codec_tag: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) coded_width: u32,
    pub(crate) coded_height: u32,
    #[serde(rename = "nb_read_frames", deserialize_with = "deserialize_str_num")]
    pub(crate) num_frames: i32,
}

fn deserialize_str_num<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StrVisitor;

    impl<'de> Visitor<'de> for StrVisitor {
        type Value = i32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            i32::from_str_radix(&value, 10).map_err(|err| E::custom(err.to_string()))
        }
    }

    deserializer.deserialize_str(StrVisitor)
}

fn deserialize_hex<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: de::Deserializer<'de>,
{
    fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
            .collect()
    }

    struct HexVisitor;

    impl<'de> Visitor<'de> for HexVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a hex string")
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if &value[0..2] != "0x" {
                return Err(E::custom(format!(
                    "expected hex value to start with \"0x\": {}",
                    value
                )));
            }

            decode_hex(&value[2..]).map_err(|err| E::custom(err.to_string()))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if &value[0..2] != "0x" {
                return Err(E::custom(format!(
                    "expected hex value to start with \"0x\": {}",
                    value
                )));
            }

            decode_hex(&value[2..]).map_err(|err| E::custom(err.to_string()))
        }
    }

    deserializer.deserialize_any(HexVisitor)
}

/// Probes a path with ffprobe and returns its properties.
pub(crate) fn probe(path: PathBuf) -> Result<ProbeInfo, String> {
    let args = [
        "-v",
        "error",
        "-count_frames",
        "-show_streams",
        "-print_format",
        "json",
    ]
    .iter()
    .map(OsString::from)
    .chain(iter::once(path.into_os_string()));

    let output = Command::new("ffprobe")
        .args(args)
        .output()
        .map_err(|err| format!("failed to execute ffprobe process: {}", err))?;

    let stderr =
        String::from_utf8(output.stderr.clone()).unwrap_or_else(|binary| format!("{:?}", binary));
    let stdout =
        String::from_utf8(output.stdout.clone()).unwrap_or_else(|binary| format!("{:?}", binary));

    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|val| val.to_string())
            .unwrap_or("Unknown".to_string());
        return Err(format!(
            "failed to execute ffprobe: exit status: {}, stderr: {}, stdout: {}",
            code, stderr, stdout
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "{}\nOutput:\n{}",
            err,
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// Probes a path with ffprobe and returns its first stream.
pub(crate) fn probe_stream(path: PathBuf) -> Result<ProbeStream, String> {
    let mut info = probe(path)?;
    if info.streams.len() != 1 {
        return Err(format!(
            "expected exactly 1 stream, got {} streams",
            info.streams.len()
        ));
    }

    Ok(info.streams.remove(0))
}

/// Fetches the first CameraInfo available.
pub(crate) fn info() -> rascam::CameraInfo {
    let mut info = rascam::info().expect("failed to query for cameras");
    assert!(info.cameras.len() >= 1, "Found 0 cameras. Exiting");
    info.cameras.remove(0)
}

/// Asserts that the actual framerate was +/- 10% of the expected framerate.
/// This is useful if the actual framerate might vary.
#[cfg(test)]
pub(crate) fn assert_framerate(actual: f32, expected: u32) {
    let actual = actual.abs();
    let expected_f32: f32 = expected as f32;
    let diff = (actual - expected_f32).abs();

    // allow +/- 10% of expected value
    let allowed_variance = expected_f32 * 0.1 + std::f32::EPSILON;

    if !actual.is_normal() {
        assert!(false, "actual framerate is not a 'normal' number (it is one of zero, infinite, subnormal, or NaN): https://doc.rust-lang.org/std/primitive.f32.html#method.is_normal");
    }

    if actual < std::f32::EPSILON && expected != 0 {
        // actual value was zero or very close
        assert!(false, "actual framerate is 0.0, expected: {}", expected);
    }

    if diff > allowed_variance {
        assert!(
            false,
            "actual framerate is {}, expected +/- 10% of {}",
            actual, expected
        );
    }
}

pub(crate) fn record_video(
    num_secs: u64,
    mut settings: CameraSettings,
    path: &str,
) -> Result<(), rascam::CameraError> {
    let info = info();

    // On the v2 camera module, the maximum resolution is 2592×1944 for stills and 1920×1080 for videos.
    if settings.width == 0 {
        settings.width = 1920;
    }
    if settings.height == 0 {
        settings.height = 1080;
    }

    let mut camera = rascam::SimpleCamera::new(info.clone())?;
    camera.configure(settings);
    camera.activate()?;

    let mut writer = BufWriter::new(File::create(path)?);
    let frame_iter = camera.take_video_frame_writer()?;

    let handle = thread::spawn(move || {
        for frame in frame_iter {
            writer.write_all(&frame[..]).unwrap();
        }
        writer
    });

    let sleep_duration = time::Duration::from_secs(num_secs);
    thread::sleep(sleep_duration);

    camera.stop();

    let mut file = handle.join().unwrap();
    file.flush().unwrap();

    Ok(())
}
