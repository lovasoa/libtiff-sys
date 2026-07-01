#![cfg(libtiff_jpeg)]

use libc::c_void;
use libtiff_sys::*;
use std::ffi::CString;
use std::path::PathBuf;

fn temp_tiff(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "libtiff-sys-jpeg-{name}-{}-{}.tif",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = std::fs::remove_file(&path);
    path
}

unsafe fn open(path: &PathBuf, mode: &str) -> *mut TIFF {
    let path = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let mode = CString::new(mode).unwrap();
    TIFFOpen(path.as_ptr(), mode.as_ptr())
}

unsafe fn write_jpeg_tiff(path: &PathBuf, width: u32, height: u32, rows_per_strip: u32) -> Vec<u8> {
    let pixels: Vec<u8> = (0..width * height).map(|v| v as u8).collect();
    write_jpeg_pixels(path, width, height, rows_per_strip, &pixels, 75);
    pixels
}

unsafe fn write_jpeg_pixels(
    path: &PathBuf,
    width: u32,
    height: u32,
    rows_per_strip: u32,
    pixels: &[u8],
    quality: u32,
) {
    assert_eq!(pixels.len(), (width * height) as usize);
    assert!(rows_per_strip > 0);

    let tif = open(path, "w");
    assert!(!tif.is_null());

    assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, width), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGELENGTH, height), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_JPEG), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, rows_per_strip), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_JPEGQUALITY, quality as i32), 1);

    let nstrips = (height + rows_per_strip - 1) / rows_per_strip;
    for strip in 0..nstrips {
        let start_row = strip * rows_per_strip;
        let rows_in_strip = std::cmp::min(rows_per_strip, height - start_row);
        let offset = (start_row * width) as usize;
        let len = (rows_in_strip * width) as usize;
        let strip_data = &pixels[offset..offset + len];
        assert_ne!(
            TIFFWriteEncodedStrip(
                tif,
                strip,
                strip_data.as_ptr() as *mut c_void,
                len as i64,
            ),
            -1
        );
    }

    TIFFClose(tif);
}

unsafe fn read_jpeg_pixels(path: &PathBuf) -> (u32, u32, Vec<u8>) {
    let tif = open(path, "r");
    assert!(!tif.is_null());

    let mut width = 0u32;
    let mut height = 0u32;
    assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
    assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);

    let mut compression = 0u16;
    assert_eq!(TIFFGetField(tif, TIFFTAG_COMPRESSION, &mut compression), 1);
    assert_eq!(compression, COMPRESSION_JPEG as u16);

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in 0..height {
        let mut scanline = vec![0u8; width as usize];
        let ret = TIFFReadScanline(tif, scanline.as_mut_ptr() as *mut c_void, row, 0);
        assert_ne!(ret, -1);
        pixels.extend(scanline);
    }

    TIFFClose(tif);
    (width, height, pixels)
}

#[test]
fn writes_and_reads_jpeg_scanlines() {
    let path = temp_tiff("jpeg-scanlines");

    unsafe {
        let pixels = write_jpeg_tiff(&path, 8, 8, 8);

        let (width, height, readback) = read_jpeg_pixels(&path);
        assert_eq!((width, height), (8, 8));
        assert_eq!(readback.len(), pixels.len());
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn jpeg_quality_tag_is_accepted() {
    let path = temp_tiff("jpeg-quality");

    unsafe {
        let pixels: Vec<u8> = (0u8..64).collect();
        write_jpeg_pixels(&path, 8, 8, 8, &pixels, 85);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let mut got_quality = 0i32;
        let got = TIFFGetField(tif, TIFFTAG_JPEGQUALITY, &mut got_quality);
        assert_eq!(got, 1);
        assert!((0..=100).contains(&got_quality), "quality out of range: {got_quality}");
        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn writes_jpeg_with_default_quality_when_tag_unset() {
    let path = temp_tiff("jpeg-default-quality");

    unsafe {
        let tif = open(&path, "w");
        assert!(!tif.is_null());

        assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, 4u32), 1);
        assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGELENGTH, 4u32), 1);
        assert_eq!(TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32), 1);
        assert_eq!(TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32), 1);
        assert_eq!(TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK), 1);
        assert_eq!(TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_JPEG), 1);

        let row: [u8; 4] = [0, 64, 128, 255];
        for r in 0..4u32 {
            assert_ne!(
                TIFFWriteScanline(tif, row.as_ptr() as *mut c_void, r, 0),
                -1
            );
        }

        TIFFClose(tif);

        let tif = open(&path, "r");
        assert!(!tif.is_null());
        let mut width = 0u32;
        let mut height = 0u32;
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);
        assert_eq!((width, height), (4, 4));

        let mut scanline = vec![0u8; 4];
        let ret = TIFFReadScanline(tif, scanline.as_mut_ptr() as *mut c_void, 0, 0);
        assert_ne!(ret, -1);

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn reads_jpeg_encoded_strips() {
    let path = temp_tiff("jpeg-strips");

    unsafe {
        let pixels: Vec<u8> = (0u8..128).collect();
        write_jpeg_pixels(&path, 8, 16, 8, &pixels, 90);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let nstrips = TIFFNumberOfStrips(tif);
        assert!(nstrips >= 1, "expected at least 1 strip, got {nstrips}");

        let strip_size = TIFFStripSize(tif);
        assert!(strip_size > 0);

        let mut buf = vec![0u8; strip_size as usize];
        let read = TIFFReadEncodedStrip(tif, 0, buf.as_mut_ptr() as *mut c_void, strip_size);
        assert!(read >= 0, "TIFFReadEncodedStrip failed");

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn decodes_jpeg_rgba_image() {
    let path = temp_tiff("jpeg-rgba");

    unsafe {
        let pixels: Vec<u8> = (0u8..64).collect();
        write_jpeg_pixels(&path, 8, 8, 8, &pixels, 80);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let mut width = 0u32;
        let mut height = 0u32;
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);

        let npixels = (width * height) as usize;
        let mut raster = vec![0u32; npixels];
        assert_eq!(
            TIFFReadRGBAImage(tif, width, height, raster.as_mut_ptr(), 0),
            1
        );
        assert!(raster.iter().any(|pixel| *pixel != 0));

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn writes_jpeg_multi_strip_and_reads_back() {
    let path = temp_tiff("jpeg-multistrip");

    unsafe {
        let pixels: Vec<u8> = (0u8..128).collect();
        write_jpeg_pixels(&path, 8, 16, 8, &pixels, 90);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let nstrips = TIFFNumberOfStrips(tif);
        assert!(nstrips >= 1);

        let mut all_data = Vec::new();
        for strip in 0..nstrips {
            let size = TIFFStripSize(tif);
            if size <= 0 {
                continue;
            }
            let mut buf = vec![0u8; size as usize];
            let read =
                TIFFReadEncodedStrip(tif, strip, buf.as_mut_ptr() as *mut c_void, size);
            assert!(read >= 0, "strip {strip} read failed");
            all_data.extend_from_slice(&buf[..read as usize]);
        }

        let mut width = 0u32;
        let mut height = 0u32;
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);

        assert!(
            !all_data.is_empty(),
            "expected non-empty decoded strip data"
        );

        TIFFClose(tif);
        let _ = (width, height);
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn writes_and_reads_jpeg_multiple_directories() {
    let path = temp_tiff("jpeg-directories");

    unsafe {
        let tif = open(&path, "w");
        assert!(!tif.is_null());

        for value in [33u8, 77u8] {
            assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, 2u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGELENGTH, 2u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_JPEG), 1);
            let row = [value, value];
            assert_ne!(TIFFWriteScanline(tif, row.as_ptr() as *mut c_void, 0, 0), -1);
            assert_ne!(TIFFWriteScanline(tif, row.as_ptr() as *mut c_void, 1, 0), -1);
            assert_eq!(TIFFWriteDirectory(tif), 1);
        }

        TIFFClose(tif);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let mut directories = 1;
        while TIFFReadDirectory(tif) == 1 {
            let mut compression = 0u16;
            assert_eq!(TIFFGetField(tif, TIFFTAG_COMPRESSION, &mut compression), 1);
            assert_eq!(compression, COMPRESSION_JPEG as u16);
            directories += 1;
        }
        assert_eq!(directories, 2);

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(&path);
}
