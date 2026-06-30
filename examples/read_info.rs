use libtiff_sys::*;
use std::ffi::CString;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: cargo run --example read_info -- <file.tif>");
    let path = CString::new(path).expect("path contains a nul byte");
    let mode = CString::new("r").unwrap();

    unsafe {
        let tif = TIFFOpen(path.as_ptr(), mode.as_ptr());
        assert!(!tif.is_null(), "failed to open input TIFF");

        let mut width = 0u32;
        let mut height = 0u32;
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &mut width), 1);
        assert_eq!(TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &mut height), 1);

        println!("{width}x{height}, {} strips", TIFFNumberOfStrips(tif));
        TIFFClose(tif);
    }
}
