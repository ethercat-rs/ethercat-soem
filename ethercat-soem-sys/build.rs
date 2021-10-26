use std::{
    env, format,
    io::{BufWriter, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

const ISSUE_224_PATCH: &[u8] = include_bytes!("issue-224.patch");

fn main() {
    let soem_dir = env::var("SOEM_PATH").unwrap_or("SOEM".to_string());

    let issue_224_workaround = env::var("CARGO_FEATURE_ISSUE_224_WORKAROUND").is_ok();

    if issue_224_workaround {
        patch_files(&soem_dir);
    }

    let dst = cmake::Config::new(&soem_dir).build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=soem");
    println!("cargo:include={}/include", dst.display());

    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}/include", dst.display()))
        .clang_arg(format!("-I{}/include/soem", dst.display()))
        .header("wrapper.h")
        .allowlist_function("ec(x?)_(.*)")
        .allowlist_type("ec_fmmu")
        .allowlist_type("ec_group")
        .allowlist_type("ec_slave")
        .allowlist_type("ec_sm")
        .allowlist_type("ec_state")
        .allowlist_type("ecx_contextt")
        .allowlist_type("ecx_portt")
        .allowlist_type("ecx_redportt")
        .opaque_type("ec_PDOassignt")
        .opaque_type("ec_PDOdesct")
        .opaque_type("ec_SMcommtypet")
        .opaque_type("ec_eepromFMMUt")
        .opaque_type("ec_eepromSMt")
        .opaque_type("ec_eringt")
        .opaque_type("ec_idxstackT")
        .opaque_type("ecx_portt")
        .opaque_type("ecx_redportt")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    if issue_224_workaround {
        undo_patch_files(&soem_dir);
    }
}

fn patch_files(soem_dir: &str) {
    let mut patch_stdin = Command::new("patch")
        .arg("-p0")
        .stdin(Stdio::piped())
        .current_dir(soem_dir)
        .spawn()
        .expect("Could not spawn 'patch' command")
        .stdin
        .unwrap();
    let mut writer = BufWriter::new(&mut patch_stdin);
    writer
        .write_all(ISSUE_224_PATCH)
        .expect("Could not patch sources");
}

fn undo_patch_files(soem_dir: &str) {
    let mut patch_stdin = Command::new("patch")
        .arg("-R")
        .arg("-p0")
        .stdin(Stdio::piped())
        .current_dir(soem_dir)
        .spawn()
        .expect("Could not spawn 'patch' command")
        .stdin
        .unwrap();
    let mut writer = BufWriter::new(&mut patch_stdin);
    writer
        .write_all(ISSUE_224_PATCH)
        .expect("Could not undo patching sources");
}
