use rascam::*;
use std::{thread, time};

// Make sure to run with --release

fn main() {
    let info = info().unwrap();
    if info.cameras.len() < 1 {
        println!("Found 0 cameras. Exiting");
        // note that this doesn't run destructors
        ::std::process::exit(1);
    }
    println!("{}", info);

    bench_jpegs_per_sec(10);
}

// Benchmarking from https://github.com/seenaburns/raytracer/blob/master/src/bench.rs

// Run function and return result with seconds duration
fn time<F, T>(f: F) -> (T, f64)
where
    F: FnOnce() -> T,
{
    let start = time::Instant::now();
    let res = f();
    let end = time::Instant::now();

    let duration = end.duration_since(start);
    let runtime_secs =
        duration.as_secs() as f64 + (duration.subsec_nanos() as f64 / 1_000_000_000.0);
    (res, runtime_secs)
}

// Prints iteration execution time and average
fn bench_jpegs_per_sec(n: i32) {
    let mut runs: Vec<f64> = Vec::with_capacity(n as usize);

    let info = info().unwrap();
    let mut camera = SimpleCamera::new(info.cameras[0].clone()).unwrap();
    camera.activate().unwrap();

    let mut b = Box::new(camera);

    let sleep_duration = time::Duration::from_millis(2000);
    thread::sleep(sleep_duration);

    for _ in 0..n {
        let images = 30;
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

fn bench_jpegs(n: i32, camera: &mut Box<SimpleCamera>) {
    for _ in 0..n {
        camera.take_one().unwrap();
    }
}
