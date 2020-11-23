use std::{
    env, format,
    io::{BufWriter, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

fn main() {
    let path = env::var("SOEM_PATH").unwrap_or("SOEM".to_string());
    let dst = cmake::Config::new(&path).build();

    if env::var("CARGO_FEATURE_ISSUE_224_WORKAROUND").is_ok() {
        let patch = include_bytes!("issue-224.patch");
        let mut patch_stdin = Command::new("patch")
            .arg("-p0")
            .stdin(Stdio::piped())
            .current_dir(path)
            .spawn()
            .expect("Could not spawn 'patch' command")
            .stdin
            .unwrap();
        let mut writer = BufWriter::new(&mut patch_stdin);
        writer.write_all(patch).expect("Could not patch sources");
    }

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=soem");
    println!("cargo:include={}/include", dst.display());

    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}/include", dst.display()))
        .clang_arg(format!("-I{}/include/soem", dst.display()))
        .header("wrapper.h")
        .whitelist_function("ec(x?)_(.*)")
        .whitelist_type("ec_fmmu")
        .whitelist_type("ec_group")
        .whitelist_type("ec_slave")
        .whitelist_type("ec_sm")
        .whitelist_type("ec_state")
        .whitelist_type("ecx_contextt")
        .whitelist_type("ecx_portt")
        .whitelist_type("ecx_redportt")
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
}
