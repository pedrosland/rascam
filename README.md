http://vojtech.kral.hk/en/rust-ffi-wrapping-c-api-in-rust-struct/
http://siciarz.net/ffi-rust-writing-bindings-libcpuid/

# API ideas

## SimpleCamera

### `SimpleCamera::new() -> Result<SimpleCamera, MMAL_STATUS_T>`

Should this actually create camera objects? (it does now)  
Should this take the camera number?  
Should this take a `MMAL_PARAMETER_CAMERA_INFO_CAMERA_T`?

### `set_camera_num(u8) -> Result((), MMAL_STATUS_T>`

If constructor doesn't take a camera number or camera info, we
should get one here.

Users or SimpleCamera shouldn't care about any of the following APIs except `take()`.

### `create_encoder() -> Result<(), MMAL_STATUS_T>`

### `enable_control_port() -> Result<(), MMAL_STATUS_T>`

### `set_camera_params(MMAL_PARAMETER_CAMERA_INFO_CAMERA_T) -> Result<(), MMAL_STATUS_T>`

Users shouldn't have to pass this in.

### `set_camera_format(MMAL_PARAMETER_CAMERA_INFO_CAMERA_T) -> Result<(), MMAL_STATUS_T>`

Users shouldn't have to pass this in and certainly not twice.

### `enable() -> Result<(), MMAL_STATUS_T>`

### `create_pool() -> Result<(), MMAL_STATUS_T>`

### `create_preview() -> Result<(), MMAL_STATUS_T>`

### `enable_preview() -> Result<(), MMAL_STATUS_T>`

### `connect_ports() -> Result<(), MMAL_STATUS_T>`

### `enable_still_port() -> Result<(), MMAL_STATUS_T>`

### `take() -> Result<(), MMAL_STATUS_T>`

Rename to capture?

## SeriousCamera (or just Camera?)

## CameraInfo

`CameraInfo::info() -> Result<CameraInfo, MMAL_STATUS_T>`

# Debugging

```
$ convert --version
Version: ImageMagick 6.9.9-27 Q16 x86_64 2017-12-23 http://www.imagemagick.org
Copyright: © 1999-2018 ImageMagick Studio LLC
License: http://www.imagemagick.org/script/license.php
Features: Cipher DPC Modules OpenMP
Delegates (built-in): bzlib cairo djvu fftw fontconfig freetype gslib jbig jng jp2 jpeg lcms ltdl lzma openexr pangocairo png ps rsvg tiff webp wmf x xml zlib
```

To test rgb output:
```
convert -size 96x96 -depth 8 -colorspace RGB rgb:test.rgb out.png
```
