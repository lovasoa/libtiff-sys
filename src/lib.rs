#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]

#[cfg(feature = "jpeg")]
extern crate mozjpeg_sys;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
