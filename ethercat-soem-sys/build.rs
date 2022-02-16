use std::{env, format, path::PathBuf};

#[cfg(feature = "issue-224-workaround")]
use std::{
    io::{BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

#[cfg(feature = "issue-224-workaround")]
const ISSUE_224_WORKAROUND_PATCH_DATA: &[u8] = include_bytes!("issue-224-workaround.patch");

fn main() {
    let soem_dir = env::var("SOEM_PATH").unwrap_or("SOEM".to_string());

    #[cfg(feature = "issue-224-workaround")]
    apply_patch(Path::new(&soem_dir), ISSUE_224_WORKAROUND_PATCH_DATA);

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

    #[cfg(feature = "issue-224-workaround")]
    unapply_patch(Path::new(&soem_dir), ISSUE_224_WORKAROUND_PATCH_DATA);
}

#[cfg(feature = "issue-224-workaround")]
fn apply_patch(soem_dir: &Path, patch_data: &[u8]) {
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
        .write_all(patch_data)
        .expect("Could not patch sources");
}

#[cfg(feature = "issue-224-workaround")]
fn unapply_patch(soem_dir: &Path, patch_data: &[u8]) {
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
        .write_all(patch_data)
        .expect("Could not undo patching sources");
}
