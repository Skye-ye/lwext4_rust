use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

fn main() {
    let c_path = PathBuf::from("c/lwext4")
        .canonicalize()
        .expect("cannot canonicalize path");

    let lwext4_make = Path::new("c/lwext4/toolchain/musl-generic.cmake");
    let lwext4_patch = Path::new("c/lwext4-make.patch").canonicalize().unwrap();

    if !Path::new(lwext4_make).exists() {
        println!("Retrieve lwext4 source code");
        let git_status = Command::new("git")
            .args(&["submodule", "update", "--init", "--recursive"])
            .status()
            .expect("failed to execute process: git submodule");
        assert!(git_status.success());

        println!("Applying patch to lwext4 src");
        // Try using the patch command directly first
        let patch_result = Command::new("patch")
            .args(&[
                "-p1",
                "-d",
                c_path.to_str().unwrap(),
                "-i",
                lwext4_patch.to_str().unwrap(),
            ])
            .status();

        match patch_result {
            Ok(status) if status.success() => {
                println!("Patch applied successfully using patch command")
            }
            _ => {
                // Fallback to manual patching if patch command fails
                println!("Patch command failed, falling back to manual patching");
                apply_patch_manually(&lwext4_patch, &c_path)
                    .expect("Failed to apply patch manually");
            }
        }

        fs::copy(
            "c/musl-generic.cmake",
            "c/lwext4/toolchain/musl-generic.cmake",
        )
        .unwrap();

        if Path::new("c/lwext4/toolchain/musl-generic.cmake").exists() {
            println!("Successfully created musl-generic.cmake file");
        } else {
            println!("Failed to create musl-generic.cmake file");
        }
    }

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let lwext4_lib = &format!("lwext4-{}", arch);
    let lwext4_lib_path = &format!("c/lwext4/lib{}.a", lwext4_lib);
    if !Path::new(lwext4_lib_path).exists() {
        let status = Command::new("make")
            .args(&[
                "musl-generic",
                "-C",
                c_path.to_str().expect("invalid path of lwext4"),
            ])
            .arg(&format!("ARCH={}", arch))
            .status()
            .expect("failed to execute process: make lwext4");
        assert!(status.success());

        let cc = &format!("{}-linux-musl-gcc", arch);
        let output = Command::new(cc)
            .args(["-print-sysroot"])
            .output()
            .expect("failed to execute process: gcc -print-sysroot");

        let sysroot = core::str::from_utf8(&output.stdout).unwrap();
        let sysroot = sysroot.trim_end();
        let sysroot_inc = &format!("-I{}/include/", sysroot);
        generates_bindings_to_rust(sysroot_inc);
    }

    println!("cargo:rustc-link-lib=static={lwext4_lib}");
    println!(
        "cargo:rustc-link-search=native={}",
        c_path.to_str().unwrap()
    );
    println!("cargo:rerun-if-changed=c/wrapper.h");
    println!("cargo:rerun-if-changed={}", c_path.to_str().unwrap());
}

/// Manually applies the specific lwext4 patch
fn apply_patch_manually(_patch_path: &Path, target_dir: &Path) -> io::Result<()> {
    println!("Applying specific lwext4 patches directly");

    // Patch 1: CMakeLists.txt
    let cmake_path = target_dir.join("CMakeLists.txt");
    if cmake_path.exists() {
        let mut content = fs::read_to_string(&cmake_path)?;

        // Replace the config flags
        content = content.replace(
            "    add_definitions(-DCONFIG_HAVE_OWN_OFLAGS=0)
    add_definitions(-DCONFIG_HAVE_OWN_ERRNO=0)
    add_definitions(-DCONFIG_HAVE_OWN_ASSERT=0)",
            "
    add_definitions(-DCONFIG_DEBUG_PRINTF=1)
    add_definitions(-DCONFIG_DEBUG_ASSERT=1)

    add_definitions(-DCONFIG_HAVE_OWN_OFLAGS=1)
    add_definitions(-DCONFIG_HAVE_OWN_ERRNO=1)
    add_definitions(-DCONFIG_HAVE_OWN_ASSERT=1)
    add_definitions(-DCONFIG_USE_USER_MALLOC=0)",
        );

        println!("Patching: {}", cmake_path.display());
        fs::write(&cmake_path, content)?;
    } else {
        println!(
            "Warning: CMakeLists.txt not found at {}",
            cmake_path.display()
        );
    }

    // Patch 2: Makefile
    let makefile_path = target_dir.join("Makefile");
    if makefile_path.exists() {
        let mut content = fs::read_to_string(&makefile_path)?;

        // Add new flags
        content = content.replace(
            "-DVERSION=$(VERSION)                                  \\",
            "-DVERSION=$(VERSION)                                  \\
\t-DLWEXT4_BUILD_SHARED_LIB=OFF \\
\t-DCMAKE_INSTALL_PREFIX=./install \\",
        );

        // Add musl-generic target
        content = content.replace(
            "endef\n\ngeneric:",
            "endef\n\nARCH ?= x86_64\n#Output: src/liblwext4.a\nmusl-generic:\n\t$(call generate_common,$@)\n\tcd build_$@ && make lwext4\n\tcp build_$@/src/liblwext4.a ./liblwext4-$(ARCH).a\n\ngeneric:"
        );

        println!("Patching: {}", makefile_path.display());
        fs::write(&makefile_path, content)?;
    } else {
        println!("Warning: Makefile not found at {}", makefile_path.display());
    }

    // Patch 3: src/CMakeLists.txt
    let src_cmake_path = target_dir.join("src/CMakeLists.txt");
    if src_cmake_path.exists() {
        let mut content = fs::read_to_string(&src_cmake_path)?;

        // Add new source file and modify build
        content = content.replace(
            "aux_source_directory(. LWEXT4_SRC)",
            "aux_source_directory(. LWEXT4_SRC)\nset(M_SRC \"../../ulibc.c\")",
        );

        content = content.replace(
            "add_library(lwext4 STATIC ${LWEXT4_SRC})",
            "add_library(lwext4 STATIC ${LWEXT4_SRC} ${M_SRC})",
        );

        println!("Patching: {}", src_cmake_path.display());
        fs::write(&src_cmake_path, content)?;
    } else {
        println!(
            "Warning: src/CMakeLists.txt not found at {}",
            src_cmake_path.display()
        );
    }

    println!("Manual patching completed");
    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn generates_bindings_to_rust(_mpath: &str) {}

#[cfg(not(target_arch = "x86_64"))]
fn generates_bindings_to_rust(mpath: &str) {
    let bindings = bindgen::Builder::default()
        .use_core()
        // The input header we would like to generate bindings for.
        .header("c/wrapper.h")
        //.clang_arg("--sysroot=/path/to/sysroot")
        .clang_arg(mpath)
        //.clang_arg("-I../../ulib/axlibc/include")
        .clang_arg("-I./c/lwext4/include")
        .clang_arg("-I./c/lwext4/build_musl-generic/include/")
        .layout_tests(false)
        // Tell cargo to invalidate the built crate whenever any of the included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from("src");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
