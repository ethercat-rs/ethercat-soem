use std::{env, format, path::PathBuf};

fn main() {
    let path = env::var("SOEM_PATH").unwrap_or("SOEM".to_string());
    let dst = cmake::Config::new(path).build();

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
