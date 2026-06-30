use libc::c_void;
use libtiff_sys::*;
use std::ffi::CString;
use std::path::PathBuf;

fn temp_tiff(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "libtiff-sys-{name}-{}-{}.tif",
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

unsafe fn write_gray_tiff(path: &PathBuf, width: u32, height: u32, rows_per_strip: u32) {
    let pixels: Vec<u8> = (0..width * height).map(|value| value as u8).collect();
    write_gray_pixels(path, width, height, rows_per_strip, &pixels);
}

unsafe fn write_gray_pixels(
    path: &PathBuf,
    width: u32,
    height: u32,
    rows_per_strip: u32,
    pixels: &[u8],
) {
    assert_eq!(pixels.len(), (width * height) as usize);

    let tif = open(path, "w");
    assert!(!tif.is_null());

    assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, width), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGELENGTH, height), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE), 1);
    assert_eq!(TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, rows_per_strip), 1);

    for row in 0..height {
        let offset = (row * width) as usize;
        let scanline = &pixels[offset..offset + width as usize];
        assert_ne!(
            TIFFWriteScanline(tif, scanline.as_ptr() as *mut c_void, row, 0),
            -1
        );
    }

    TIFFClose(tif);
}

unsafe fn read_gray_pixels(path: &PathBuf) -> (u32, u32, Vec<u8>) {
    let tif = open(path, "r");
    assert!(!tif.is_null());

    let mut width = 0u32;
    let mut height = 0u32;
    assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
    assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in 0..height {
        let mut scanline = vec![0u8; width as usize];
        assert_ne!(
            TIFFReadScanline(tif, scanline.as_mut_ptr() as *mut c_void, row, 0),
            -1
        );
        pixels.extend(scanline);
    }

    TIFFClose(tif);
    (width, height, pixels)
}

#[test]
fn reports_version() {
    let version = unsafe { TIFFGetVersion() };
    assert!(!version.is_null());
}

#[test]
fn writes_and_reads_scanlines() {
    let path = temp_tiff("scanlines");

    unsafe {
        write_gray_tiff(&path, 4, 3, 1);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let mut width = 0u32;
        let mut height = 0u32;
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);
        assert_eq!((width, height), (4, 3));

        let mut got = Vec::new();
        for row in 0..height {
            let mut scanline = vec![0u8; width as usize];
            assert_ne!(
                TIFFReadScanline(tif, scanline.as_mut_ptr() as *mut c_void, row, 0),
                -1
            );
            got.extend(scanline);
        }

        TIFFClose(tif);
        assert_eq!(got, (0u8..12).collect::<Vec<_>>());
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn roundtrips_scanline_pixels_through_a_second_tiff() {
    let source = temp_tiff("roundtrip-source");
    let copy = temp_tiff("roundtrip-copy");
    let expected: Vec<u8> = (0..24).map(|value| ((value * 7) % 251) as u8).collect();

    unsafe {
        write_gray_pixels(&source, 6, 4, 2, &expected);

        let (width, height, pixels) = read_gray_pixels(&source);
        assert_eq!((width, height), (6, 4));
        assert_eq!(pixels, expected);

        write_gray_pixels(&copy, width, height, 2, &pixels);

        let (copy_width, copy_height, copy_pixels) = read_gray_pixels(&copy);
        assert_eq!((copy_width, copy_height), (width, height));
        assert_eq!(copy_pixels, expected);
    }

    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(copy);
}

#[test]
fn reads_encoded_strips() {
    let path = temp_tiff("strips");

    unsafe {
        write_gray_tiff(&path, 5, 4, 2);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        assert_eq!(TIFFNumberOfStrips(tif), 2);
        let strip_size = TIFFStripSize(tif);
        assert!(strip_size >= 10);

        let mut first_strip = vec![0u8; strip_size as usize];
        let read = TIFFReadEncodedStrip(
            tif,
            0,
            first_strip.as_mut_ptr() as *mut c_void,
            strip_size,
        );
        assert_eq!(read, 10);
        assert_eq!(&first_strip[..10], &(0u8..10).collect::<Vec<_>>());

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn decodes_rgba_image() {
    let path = temp_tiff("rgba");

    unsafe {
        write_gray_tiff(&path, 2, 2, 1);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let mut raster = vec![0u32; 4];
        assert_eq!(TIFFReadRGBAImage(tif, 2, 2, raster.as_mut_ptr(), 0), 1);
        assert!(raster.iter().any(|pixel| *pixel != 0));

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn writes_and_counts_multiple_directories() {
    let path = temp_tiff("directories");

    unsafe {
        let tif = open(&path, "w");
        assert!(!tif.is_null());

        for value in [11u8, 22u8] {
            assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, 1u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_IMAGELENGTH, 1u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG), 1);
            assert_eq!(TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE), 1);
            assert_ne!(TIFFWriteScanline(tif, &value as *const u8 as *mut c_void, 0, 0), -1);
            assert_eq!(TIFFWriteDirectory(tif), 1);
        }

        TIFFClose(tif);

        let tif = open(&path, "r");
        assert!(!tif.is_null());

        let mut directories = 1;
        while TIFFReadDirectory(tif) == 1 {
            directories += 1;
        }
        assert_eq!(directories, 2);

        TIFFClose(tif);
    }

    let _ = std::fs::remove_file(path);
}
