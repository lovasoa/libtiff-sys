use libc::c_void;
use libtiff_sys::*;
use std::ffi::CString;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "example-gray.tif".to_string());
    let path = CString::new(path).expect("path contains a nul byte");
    let mode = CString::new("w").unwrap();

    unsafe {
        let tif = TIFFOpen(path.as_ptr(), mode.as_ptr());
        assert!(!tif.is_null(), "failed to open output TIFF");

        let width = 8u32;
        let height = 8u32;
        TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, width);
        TIFFSetField(tif, TIFFTAG_IMAGELENGTH, height);
        TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1u16 as u32);
        TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, 8u16 as u32);
        TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
        TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
        TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
        TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, 1u32);

        for row in 0..height {
            let scanline: Vec<u8> = (0..width).map(|col| ((row + col) * 16) as u8).collect();
            let status = TIFFWriteScanline(tif, scanline.as_ptr() as *mut c_void, row, 0);
            assert_ne!(status, -1, "failed to write scanline {row}");
        }

        TIFFClose(tif);
    }
}
