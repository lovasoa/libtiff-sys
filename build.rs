use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=TIFF_LIB_DIR");
    println!("cargo:rerun-if-env-changed=TIFF_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=TIFF_STATIC");
    println!("cargo:rerun-if-env-changed=TIFF_DYNAMIC");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=libtiff");

    let link_static = link_static();
    let include_dir = if let Some(lib_dir) = env::var_os("TIFF_LIB_DIR") {
        let include_dir = env::var_os("TIFF_INCLUDE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("libtiff/libtiff"));
        println!("cargo:rustc-link-search=native={}", PathBuf::from(lib_dir).display());
        print_link_lib(link_static);
        include_dir
    } else if let Some(include_dir) = try_pkg_config(link_static) {
        include_dir
    } else {
        build_bundled_libtiff()
    };

    println!("cargo:include={}", include_dir.display());
    generate_bindings(&include_dir);
}

fn link_static() -> bool {
    if env::var_os("TIFF_STATIC").is_some() {
        return true;
    }
    if env::var_os("TIFF_DYNAMIC").is_some() {
        return false;
    }
    if env::var_os("CARGO_FEATURE_STATIC").is_some() {
        return true;
    }
    if env::var_os("CARGO_FEATURE_DYNAMIC").is_some() {
        return false;
    }

    matches!(
        env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("macos") | Ok("windows")
    ) || env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("musl")
}

fn try_pkg_config(link_static: bool) -> Option<PathBuf> {
    let mut config = pkg_config::Config::new();
    config.statik(link_static);

    let library = config.probe("libtiff-4").or_else(|_| config.probe("libtiff")).ok()?;
    library.include_paths.into_iter().next()
}

fn build_bundled_libtiff() -> PathBuf {
    let mut config = cmake::Config::new("libtiff");
    config
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("tiff-tools", "OFF")
        .define("tiff-tests", "OFF")
        .define("tiff-contrib", "OFF")
        .define("tiff-docs", "OFF")
        .define("tiff-deprecated", "OFF")
        .define("tiff-cxx", "OFF")
        .define("zlib", "OFF")
        .define("deflate", "OFF")
        .define("jpeg", "OFF")
        .define("jbig", "OFF")
        .define("lerc", "OFF")
        .define("lzma", "OFF")
        .define("zstd", "OFF")
        .define("webp", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        config.define("CMAKE_REQUIRED_LIBRARIES", "m");
    }

    let dst = config.build();

    println!("cargo:rustc-link-search=native={}", dst.join("lib").display());
    println!("cargo:rustc-link-search=native={}", dst.join("lib64").display());
    print_bundled_link_lib();

    dst.join("include")
}

fn print_bundled_link_lib() {
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && env::var("PROFILE").as_deref() == Ok("debug")
    {
        println!("cargo:rustc-link-lib=static=tiffd");
    } else {
        println!("cargo:rustc-link-lib=static=tiff");
    }
}

fn print_link_lib(link_static: bool) {
    if link_static {
        println!("cargo:rustc-link-lib=static=tiff");
    } else {
        println!("cargo:rustc-link-lib=tiff");
    }
}

fn generate_bindings(include_dir: &Path) {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .generate_comments(false)
        .layout_tests(false)
        .allowlist_function("TIFF.*")
        .allowlist_function("_TIFF.*")
        .allowlist_type("TIFF.*")
        .allowlist_var("TIFF.*")
        .allowlist_var("COMPRESSION_.*")
        .allowlist_var("PHOTOMETRIC_.*")
        .allowlist_var("PLANARCONFIG_.*")
        .allowlist_var("ORIENTATION_.*")
        .allowlist_var("SAMPLEFORMAT_.*")
        .allowlist_var("EXTRASAMPLE_.*")
        .generate()
        .expect("failed to generate libtiff bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write libtiff bindings");
}
