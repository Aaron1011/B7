extern crate bindgen;
extern crate cc;
extern crate num_cpus;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    println!("cargo:rerun-if-changed=src/bindgen.h");
    //println!("cargo:rerun-if-changed=dynamorio/");
    let mut out_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    out_dir.push("dynamorio");
    out_dir.push("build");
    /*for entry in WalkDir::new("dynamorio") {
        println!("cargo:rerun-if-changed={}", entry.unwrap().path().display());
    }*/

    fs::create_dir_all(&out_dir).expect("Failed to make output dir");
    let cpus = num_cpus::get();

    if !Command::new("cmake")
        .args(&["..", "-DDISABLE_WARNINGS=yes"])
        .current_dir(&out_dir)
        .spawn()
        .expect("Failed to spawn cmake")
        .wait()
        .expect("Failed to run cmake")
        .success()
    {
        panic!("cmake failed!");
    }

    if !Command::new("make")
        .args(&["-j", &format!("{}", cpus)])
        .current_dir(out_dir)
        .spawn()
        .expect("Failed to spawn make")
        .wait()
        .expect("Failed to run make")
        .success()
    {
        panic!("make failed!");
    }

    // Generate Rust bindings
    let bindings = bindgen::Builder::default()
        .header("src/bindgen.h")
        .whitelist_type("perf_event_attr")
        .whitelist_type("perf_type_id")
        .whitelist_type("perf_hw_id")
        .generate()
        .expect("Unable to generate bindings");

    // Output rust bindings to a file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
