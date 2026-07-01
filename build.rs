use std::env;
use std::fs;
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

    #[cfg(feature = "jpeg")]
    {
        println!("cargo:rustc-cfg=libtiff_jpeg");
    }
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

fn apply_bundled_cmake_defines(config: &mut cmake::Config) {
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
        .define("jbig", "OFF")
        .define("lerc", "OFF")
        .define("lzma", "OFF")
        .define("zstd", "OFF")
        .define("webp", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        config.define("CMAKE_REQUIRED_LIBRARIES", "m");
    }
}

fn finish_bundled_build(dst: &Path) -> PathBuf {
    println!("cargo:rustc-link-search=native={}", dst.join("lib").display());
    println!("cargo:rustc-link-search=native={}", dst.join("lib64").display());
    print_bundled_link_lib();
    dst.join("include")
}

#[cfg(feature = "jpeg")]
fn find_mozjpeg_artifacts() -> (PathBuf, PathBuf, Vec<PathBuf>) {
    let include_var = env::var_os("DEP_JPEG_INCLUDE")
        .expect("DEP_JPEG_INCLUDE should be set by mozjpeg-sys");
    let include_paths: Vec<PathBuf> = env::split_paths(&include_var).collect();

    let vendor_dir = include_paths
        .iter()
        .find(|p| p.join("jpeglib.h").exists())
        .expect(
            "Could not find jpeglib.h in mozjpeg-sys include paths. \
             Make sure mozjpeg-sys is built with its default features.",
        );

    let config_dir = include_paths
        .iter()
        .find(|p| p.join("jconfig.h").exists())
        .expect("Could not find jconfig.h in mozjpeg-sys include paths.");

    // Merge all headers into a staging directory so cmake finds everything
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let staging = out_dir.join("jpeg-include");
    let _ = fs::create_dir_all(&staging);

    for dir in &[vendor_dir, config_dir] {
        for entry in fs::read_dir(dir).expect("failed to read mozjpeg include dir") {
            let entry = entry.expect("failed to read entry");
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let target = staging.join(entry.file_name());
            if !target.exists() {
                fs::copy(&path, &target).expect("failed to copy mozjpeg header");
            }
        }
    }

    // Find compiled mozjpeg libraries (libmozjpeg*.a / mozjpeg*.lib)
    let build_dir = config_dir
        .parent()
        .expect("config dir should have parent");
    let mut libs: Vec<PathBuf> = fs::read_dir(build_dir)
        .expect("failed to read mozjpeg build dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| {
                    (n.starts_with("libmozjpeg") || n.starts_with("mozjpeg"))
                        && (n.ends_with(".a") || n.ends_with(".lib"))
                })
        })
        .collect();
    libs.sort();

    // Main library is the one without "simd" in the name (SIMD objects are linked in
    // via NASM on x86_64; on ARM a separate libmozjpegsimd*.a may exist).
    let main_lib = libs
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| !n.contains("simd"))
        })
        .or_else(|| libs.first())
        .expect("Could not find compiled mozjpeg library in build directory")
        .clone();

    let extra_libs: Vec<PathBuf> = libs.iter().filter(|p| *p != &main_lib).cloned().collect();

    (main_lib, staging, extra_libs)
}

#[cfg(feature = "jpeg")]
fn build_bundled_libtiff_with_jpeg(source_dir: &Path) -> PathBuf {
    let (jpeg_lib, jpeg_include, extra_libs) = find_mozjpeg_artifacts();

    // Create a cmake wrapper that pre-creates the JPEG::JPEG target so libtiff's
    // built-in FindJPEG module sees it and skips searching.
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let wrapper_dir = out_dir.join("libtiff-cmake-wrapper");
    let _ = fs::create_dir_all(&wrapper_dir);

    let mut content = String::new();
    content.push_str(&format!(
        "cmake_minimum_required(VERSION 3.10)\n\
         project(tiff_wrapper C)\n\
         set(CMAKE_C_STANDARD 99)\n\
         \n\
         set(tiff-install ON CACHE BOOL \"\" FORCE)\n\
         set(JPEG_FOUND TRUE CACHE BOOL \"\" FORCE)\n\
         set(JPEG_LIBRARY \"{lib}\" CACHE FILEPATH \"\" FORCE)\n\
         set(JPEG_INCLUDE_DIR \"{inc}\" CACHE PATH \"\" FORCE)\n\
         set(JPEG_INCLUDE_DIRS \"{inc}\" CACHE STRING \"\" FORCE)\n\
         set(JPEG_LIBRARIES \"{lib}\" CACHE STRING \"\" FORCE)\n",
        lib = jpeg_lib.display(),
        inc = jpeg_include.display(),
    ));

    if !extra_libs.is_empty() {
        let extra: Vec<String> = extra_libs.iter().map(|p| p.display().to_string()).collect();
        content.push_str(&format!(
            "set(JPEG_EXTRA_LIBS \"{}\" CACHE STRING \"\" FORCE)\n",
            extra.join(";")
        ));
    }

    content.push_str(
        "\n\
         add_library(JPEG::JPEG UNKNOWN IMPORTED)\n\
         set_target_properties(JPEG::JPEG PROPERTIES\n\
             IMPORTED_LOCATION \"${JPEG_LIBRARY}\"\n\
             INTERFACE_INCLUDE_DIRECTORIES \"${JPEG_INCLUDE_DIR}\")\n",
    );

    if !extra_libs.is_empty() {
        content.push_str(
            "set_target_properties(JPEG::JPEG PROPERTIES\n\
             INTERFACE_LINK_LIBRARIES \"${JPEG_EXTRA_LIBS}\")\n",
        );
    }

    content.push_str(&format!(
        "\nadd_subdirectory(\"{src}\" tiff)\n",
        src = source_dir.display()
    ));

    fs::write(wrapper_dir.join("CMakeLists.txt"), content)
        .expect("failed to write JPEG cmake wrapper");

    let mut config = cmake::Config::new(&wrapper_dir);
    apply_bundled_cmake_defines(&mut config);
    let dst = config.build();
    finish_bundled_build(&dst)
}

fn build_bundled_libtiff() -> PathBuf {
    let source_dir = prepare_bundled_libtiff_source();

    #[cfg(feature = "jpeg")]
    {
        return build_bundled_libtiff_with_jpeg(&source_dir);
    }

    #[cfg(not(feature = "jpeg"))]
    {
        let mut config = cmake::Config::new(&source_dir);
        config.define("jpeg", "OFF");
        apply_bundled_cmake_defines(&mut config);
        let dst = config.build();
        finish_bundled_build(&dst)
    }
}

fn prepare_bundled_libtiff_source() -> PathBuf {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let source_dir = out_dir.join("libtiff-src");
    if source_dir.exists() {
        fs::remove_dir_all(&source_dir).expect("failed to clean bundled libtiff source copy");
    }
    copy_dir(Path::new("libtiff"), &source_dir);

    let cmake_lists = source_dir.join("CMakeLists.txt");
    let contents = fs::read_to_string(&cmake_lists).expect("failed to read libtiff CMakeLists.txt");
    fs::write(&cmake_lists, contents.replace("LANGUAGES C CXX", "LANGUAGES C"))
        .expect("failed to patch libtiff CMakeLists.txt");

    source_dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("failed to create bundled libtiff source copy");
    for entry in fs::read_dir(from).expect("failed to read bundled libtiff source") {
        let entry = entry.expect("failed to read bundled libtiff source entry");
        let file_type = entry.file_type().expect("failed to read bundled libtiff file type");
        let target = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if file_type.is_file() {
            fs::copy(entry.path(), target).expect("failed to copy bundled libtiff source file");
        }
    }
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
