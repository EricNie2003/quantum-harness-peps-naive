use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

fn command_or_panic(program: &OsString, args: &[OsString], label: &str) {
    let status = Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|error| panic!("failed to start {label}: {error}"));
    assert!(status.success(), "{label} failed with status {status}");
}

fn main() {
    println!("cargo:rerun-if-changed=cuda/nqueens_gpu.cu");
    println!("cargo:rerun-if-changed=cuda/nqueens_gpu.h");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=NVCC");
    println!("cargo:rerun-if-env-changed=NQUEENS_CUDA_SKIP_NATIVE");

    if env::var_os("CARGO_FEATURE_CUDA").is_none() {
        return;
    }
    if env::var_os("NQUEENS_CUDA_SKIP_NATIVE").as_deref() == Some(OsStr::new("1")) {
        println!("cargo:warning=skipping native CUDA compilation for Rust-only type checking");
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("Cargo sets CARGO_CFG_TARGET_OS");
    assert_eq!(
        target_os, "linux",
        "the E11 CUDA backend currently supports Linux and Ubuntu/WSL2"
    );

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let object = out_dir.join("nqueens_gpu.o");
    let archive = out_dir.join("libnqueens_gpu.a");
    let nvcc = env::var_os("NVCC").unwrap_or_else(|| OsString::from("nvcc"));

    let compile_args = vec![
        OsString::from("-std=c++17"),
        OsString::from("-O3"),
        OsString::from("-lineinfo"),
        OsString::from("-Xcompiler"),
        OsString::from("-fPIC"),
        OsString::from("-gencode"),
        OsString::from("arch=compute_89,code=sm_89"),
        OsString::from("-gencode"),
        OsString::from("arch=compute_90,code=sm_90"),
        OsString::from("-gencode"),
        OsString::from("arch=compute_90,code=compute_90"),
        OsString::from("-c"),
        OsString::from("cuda/nqueens_gpu.cu"),
        OsString::from("-o"),
        object.clone().into_os_string(),
    ];
    command_or_panic(&nvcc, &compile_args, "nvcc CUDA compilation");

    let ar = env::var_os("AR").unwrap_or_else(|| OsString::from("ar"));
    let archive_args = vec![
        OsString::from("crs"),
        archive.clone().into_os_string(),
        object.into_os_string(),
    ];
    command_or_panic(&ar, &archive_args, "CUDA static archive creation");

    let cuda_home = env::var_os("CUDA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local/cuda"));
    let lib64 = cuda_home.join("lib64");
    let target_lib = cuda_home.join("targets/x86_64-linux/lib");
    let cuda_lib = if lib64.is_dir() { lib64 } else { target_lib };
    assert!(
        Path::new(&cuda_lib).is_dir(),
        "CUDA runtime library directory not found below {}; set CUDA_HOME",
        cuda_home.display()
    );

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-search=native={}", cuda_lib.display());
    println!("cargo:rustc-link-lib=static=nqueens_gpu");
    println!("cargo:rustc-link-lib=dylib=cudart");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
