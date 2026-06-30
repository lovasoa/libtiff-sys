# libtiff-sys

Low-level Rust FFI bindings for `libtiff`.

The crate links to `libtiff` and exposes declarations generated from `tiffio.h`. It intentionally stays close to the C API: functions are unsafe, handles must be closed with `TIFFClose`, and varargs functions such as `TIFFSetField` and `TIFFGetField` must be called with the exact C-compatible argument types expected by libtiff.

Higher-level safe APIs should be built in a separate wrapper crate.

## Build Behavior

The build script follows the usual `-sys` crate model:

- Use `TIFF_LIB_DIR` when an explicit library location is provided.
- Otherwise try `pkg-config` for `libtiff-4` or `libtiff`.
- Otherwise build the bundled `libtiff` source with CMake inside Cargo's `OUT_DIR` and link it statically.

The build script also exports `DEP_TIFF_INCLUDE` for crates that need to compile C code against the same headers.

## Configuration

- `TIFF_LIB_DIR`: directory containing a prebuilt `libtiff` library.
- `TIFF_INCLUDE_DIR`: directory containing `tiffio.h` and related headers.
- `TIFF_STATIC=1`: request static linking.
- `TIFF_DYNAMIC=1`: request dynamic linking.
- Cargo features `static` and `dynamic` are also supported when environment variables are not set.

Environment variables take precedence over Cargo features. If neither static nor dynamic linking is requested, the crate defaults to static linking on macOS, Windows, and musl targets, and dynamic linking elsewhere when a system library is found.

## Examples

Write a small 8-bit grayscale TIFF:

```sh
cargo run --example write_gray -- /tmp/example-gray.tif
```

Read basic image metadata:

```sh
cargo run --example read_info -- /tmp/example-gray.tif
```

A minimal write flow looks like this:

```rust,no_run
use libc::c_void;
use libtiff_sys::*;
use std::ffi::CString;

unsafe {
    let path = CString::new("out.tif").unwrap();
    let mode = CString::new("w").unwrap();
    let tif = TIFFOpen(path.as_ptr(), mode.as_ptr());
    assert!(!tif.is_null());

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, 1u32);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, 1u32);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);

    let pixel = [128u8];
    TIFFWriteScanline(tif, pixel.as_ptr() as *mut c_void, 0, 0);
    TIFFClose(tif);
}
```

## Testing

The test suite writes and reads real TIFF files through libtiff:

- library version/linking check
- scanline write/read round trip
- encoded strip reads
- RGBA decoding
- multi-directory TIFF traversal

Run it with:

```sh
cargo test
```
