http://vojtech.kral.hk/en/rust-ffi-wrapping-c-api-in-rust-struct/
http://siciarz.net/ffi-rust-writing-bindings-libcpuid/

Debugging:

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
