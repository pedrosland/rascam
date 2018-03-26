// For some reason I don't understand, this is required for `cargo test` but not `cargo build`.

fn main() {
    println!("cargo:rustc-link-lib=mmal_core");
    println!("cargo:rustc-link-lib=mmal_util");
    println!("cargo:rustc-link-lib=mmal_vc_client");
    println!("cargo:rustc-link-lib=vcos");
    println!("cargo:rustc-link-lib=bcm_host");
    println!("cargo:rustc-link-search=native=/opt/vc/lib");
}
