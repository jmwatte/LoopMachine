fn main() {
    // Alleen Rubber Band compileren als de feature aan staat
    #[cfg(feature = "rubberband")]
    build_rubberband();

    // Windows icoon embedden
    #[cfg(target_os = "windows")]
    build_icon();
}

#[cfg(target_os = "windows")]
fn build_icon() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ico_path = std::path::Path::new(&manifest).join("loopMachine.ico");

    if ico_path.exists() {
        println!("cargo:rerun-if-changed={}", ico_path.display());
        let inc = format!("/i{}", manifest.replace("/", "\\"));
        let _ = embed_resource::compile("app.rc", &[&inc as &str] as &[&str]);
    }
}

#[cfg(feature = "rubberband")]
fn build_rubberband() {
    let vendor = std::path::Path::new("vendor/rubberband");

    let mut build = cc::Build::new();

    // C++ bestanden
    build.cpp(true);

    // C++ standaard: MSVC gebruikt een andere vlag
    if cfg!(target_os = "windows") {
        // MSVC default is al C++14, geen aparte vlag nodig
        build.flag("/std:c++14");
    } else {
        build.std("c++11");
        build.flag_if_supported("-Wno-unused-parameter");
        build.flag_if_supported("-Wno-unused-variable");
    }

    // Include directories
    build
        .include(vendor.join("rubberband")) // publieke headers (rubberband-c.h e.d.)
        .include(vendor.join("src")); // interne headers

    // Defines voor Windows
    if cfg!(target_os = "windows") {
        build
            .define("NOMINMAX", None)
            .define("_USE_MATH_DEFINES", None)
            .define("_CRT_SECURE_NO_WARNINGS", None);
    }

    // Rubber Band compileert statisch → geen dllimport
    build.define("RUBBERBAND_STATIC", None);

    // Gebruik ingebouwde FFT en BQResampler (standaard, geen externe deps nodig)
    build.define("USE_BUILTIN_FFT", None);
    build.define("USE_BQRESAMPLER", None);

    // --- Library source files ---
    // src/ (C API wrapper + main stretcher)
    build.file(vendor.join("src/rubberband-c.cpp"));
    build.file(vendor.join("src/RubberBandStretcher.cpp"));
    build.file(vendor.join("src/RubberBandLiveShifter.cpp"));

    // src/faster/
    build.file(vendor.join("src/faster/AudioCurveCalculator.cpp"));
    build.file(vendor.join("src/faster/CompoundAudioCurve.cpp"));
    build.file(vendor.join("src/faster/HighFrequencyAudioCurve.cpp"));
    build.file(vendor.join("src/faster/SilentAudioCurve.cpp"));
    build.file(vendor.join("src/faster/PercussiveAudioCurve.cpp"));
    build.file(vendor.join("src/faster/R2Stretcher.cpp"));
    build.file(vendor.join("src/faster/StretcherChannelData.cpp"));
    build.file(vendor.join("src/faster/StretcherProcess.cpp"));

    // src/common/
    build.file(vendor.join("src/common/Allocators.cpp"));
    build.file(vendor.join("src/common/BQResampler.cpp"));
    build.file(vendor.join("src/common/FFT.cpp"));
    build.file(vendor.join("src/common/Log.cpp"));
    build.file(vendor.join("src/common/Profiler.cpp"));
    build.file(vendor.join("src/common/Resampler.cpp"));
    build.file(vendor.join("src/common/StretchCalculator.cpp"));
    build.file(vendor.join("src/common/sysutils.cpp"));
    build.file(vendor.join("src/common/mathmisc.cpp"));
    build.file(vendor.join("src/common/Thread.cpp"));

    // src/finer/
    build.file(vendor.join("src/finer/R3Stretcher.cpp"));
    build.file(vendor.join("src/finer/R3LiveShifter.cpp"));

    // --- Extra C sources voor Windows (getopt) ---
    if cfg!(target_os = "windows") {
        build.file(vendor.join("src/ext/getopt/getopt.c"));
        build.file(vendor.join("src/ext/getopt/getopt_long.c"));
    }

    build.compile("rubberband");
}
