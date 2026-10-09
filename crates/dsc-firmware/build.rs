//! Builds the DscDevice library from firmware/ for the PC, with stand-ins for
//! the Arduino core and TinyUSB from native/.

fn main() {
    let firmware = "../../firmware/DscDevice/src";
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include("native")
        .include(firmware)
        .define("USE_TINYUSB", None)
        .file(format!("{firmware}/DscDevice.cpp"))
        .file(format!("{firmware}/DscHid.cpp"))
        .file("native/shim.cpp");
    // AddressSanitizer: any read or write past a buffer crashes the test.
    if std::env::var_os("DSC_ASAN").is_some() {
        build.flag("/fsanitize=address");
    }
    build.compile("dscfirmware");
    println!("cargo:rerun-if-changed={firmware}");
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-env-changed=DSC_ASAN");
}
