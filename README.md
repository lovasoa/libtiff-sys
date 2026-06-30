# libtiff-sys

Low-level Rust FFI bindings for `libtiff`.

This crate links Rust code to the C `libtiff` library and exposes bindings generated from `tiffio.h`. It intentionally stays close to the C API: functions are unsafe, handles must be closed with `TIFFClose`, and varargs functions such as `TIFFSetField` and `TIFFGetField` must be called with the exact C-compatible argument types expected by libtiff.

Higher-level safe APIs should be built in a separate wrapper crate.

## Supported Platforms

The crate is tested in CI on:

- Linux GNU, native `x86_64-unknown-linux-gnu`
- Linux musl, cross `x86_64-unknown-linux-musl`
- Linux GNU, cross `aarch64-unknown-linux-gnu`
- macOS, native Apple Silicon runner
- macOS, cross `aarch64-apple-darwin`
- Windows MSVC, native `x86_64-pc-windows-msvc`
- Windows MSVC, cross-check `x86_64-pc-windows-msvc`

Other targets may work if CMake, Clang/libclang, a C compiler, and either a target-compatible system `libtiff` or the bundled source build are available.

## Build Modes

The build script chooses one of three modes, in this order:

1. Explicit system library mode: use `TIFF_LIB_DIR` and optionally `TIFF_INCLUDE_DIR`.
2. `pkg-config` system library mode: probe `libtiff-4`, then `libtiff`.
3. Bundled source mode: build the bundled `libtiff` source with CMake inside Cargo's `OUT_DIR` and link it statically.

The build script exports `DEP_TIFF_INCLUDE` for dependent crates that need to compile C code against the same headers.

## Linking Defaults

If you do not request a linking mode explicitly:

- Linux and BSD GNU targets prefer dynamic linking when a system library is found through `pkg-config`.
- macOS defaults to static linking.
- Windows defaults to static linking.
- musl targets default to static linking.
- bundled source builds are always static.

Environment variables take precedence over Cargo features.

## Configuration

Use environment variables for top-level build control:

- `TIFF_LIB_DIR`: directory containing a prebuilt `libtiff` library.
- `TIFF_INCLUDE_DIR`: directory containing `tiffio.h` and related headers. If omitted with `TIFF_LIB_DIR`, the bundled headers are used for binding generation.
- `TIFF_STATIC=1`: request static linking.
- `TIFF_DYNAMIC=1`: request dynamic linking.
- `LIBTIFF_4_NO_PKG_CONFIG=1` or `LIBTIFF_NO_PKG_CONFIG=1`: disable `pkg-config` probing and force the bundled fallback unless `TIFF_LIB_DIR` is set.

Cargo features are also available for simple dependency configuration:

- `static`: request static linking when no env var overrides it.
- `dynamic`: request dynamic linking when no env var overrides it.

Do not enable both `static` and `dynamic`. If both are enabled, `static` currently wins because Cargo features are additive and cannot be unset by downstream crates.

## Build-Time Dependencies

All builds need:

- Rust stable
- Cargo
- Clang/libclang, required by `bindgen`
- A C compiler for bundled builds and most cross builds

Bundled source mode also needs:

- CMake
- A target-compatible C compiler
- C standard library headers for the target

System library modes also need:

- `libtiff` headers, including `tiffio.h`
- A target-compatible `libtiff` library
- `pkg-config` when using automatic system discovery

Example Linux packages:

```sh
sudo apt-get install clang libclang-dev cmake pkg-config libtiff-dev
```

Example macOS packages:

```sh
brew install llvm cmake pkg-config libtiff
```

Windows MSVC builds require Visual Studio Build Tools. Bundled mode is designed to avoid requiring a separate `libtiff` installation.

## Runtime Dependencies

Runtime requirements depend on the selected link mode:

- Static bundled mode: no separate `libtiff` dynamic library is required.
- Static system mode: no separate `libtiff` dynamic library is required, but any static dependencies selected by the system `libtiff.pc` must also be linkable.
- Dynamic system mode: the target machine must have a compatible `libtiff` shared library available to the dynamic loader.

If dynamic linking succeeds at build time but your binary fails to start, check platform loader paths such as `LD_LIBRARY_PATH`, `DYLD_LIBRARY_PATH`, `PATH`, rpath, or your package manager's runtime dependency metadata.

## Bundled libtiff Features

Bundled mode intentionally disables optional external codecs by default:

- zlib/deflate
- JPEG
- JBIG
- LERC
- LZMA
- ZSTD
- WebP

This keeps the fallback build self-contained and avoids accidentally detecting host libraries that are not linked into Rust binaries, which is a common `-sys` crate failure mode. If you need those codecs, use a system `libtiff` built with the desired features and point this crate at it through `pkg-config` or `TIFF_LIB_DIR`/`TIFF_INCLUDE_DIR`.

## Cross-Compilation

For cross-compilation, prefer one of these approaches:

- Use bundled mode with a target-compatible C compiler and CMake toolchain configuration.
- Use `TIFF_LIB_DIR` and `TIFF_INCLUDE_DIR` pointing at a target-compatible `libtiff` install.
- Use `pkg-config` only when it is configured for the target sysroot, not the host.

Useful environment variables include:

- `CC_<target>`: C compiler for the target, for example `CC_aarch64-unknown-linux-gnu=aarch64-linux-gnu-gcc`.
- `CARGO_TARGET_<TARGET>_LINKER`: Rust linker for the target, for example `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc`.
- `CMAKE_TOOLCHAIN_FILE`: CMake toolchain file for non-trivial cross builds.
- `PKG_CONFIG_SYSROOT_DIR`, `PKG_CONFIG_LIBDIR`, and `PKG_CONFIG_ALLOW_CROSS=1`: only when using target-aware `pkg-config`.

Do not let `pkg-config` find host `libtiff` while compiling for another target. Disable it with `LIBTIFF_4_NO_PKG_CONFIG=1` or `LIBTIFF_NO_PKG_CONFIG=1` if in doubt.

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
- scanline copy into a second TIFF and readback
- encoded strip reads
- RGBA decoding
- multi-directory TIFF traversal

Run it with:

```sh
cargo test
```

The GitHub Actions matrix also checks:

- bundled static builds on Linux, macOS, and Windows
- system `pkg-config` dynamic builds
- system `pkg-config` static builds
- examples that create and inspect real TIFF files
- package tarball verification
- building without the git submodule when an explicit system `libtiff` is available
- representative cross-compilation targets

## Troubleshooting

If `bindgen` cannot find libclang, install Clang/libclang and set `LIBCLANG_PATH` to the directory containing the libclang shared library.

If linking fails with missing symbols from codecs such as zlib, zstd, jpeg, or lzma, your system `libtiff` was built with optional dependencies that were not linked. Use the bundled mode, install the matching development packages, or fix the `libtiff.pc` metadata used by `pkg-config`.

If cross-compilation finds the wrong headers or libraries, disable `pkg-config` and either use bundled mode or set `TIFF_LIB_DIR` and `TIFF_INCLUDE_DIR` to target sysroot paths.

If dynamic binaries fail at runtime, verify the platform dynamic loader can find `libtiff` and any libraries it depends on.
