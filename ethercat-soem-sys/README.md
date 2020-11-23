# ethercat-soem-sys

Rust FFI bindings for [SOEM](https://github.com/OpenEtherCATsociety/SOEM).

## Usage

```toml
[dependencies]
ethercat-soem-sys = "*"
```

By default this crate compiles with the upstream SOEM master
([commit 342ca86](https://github.com/OpenEtherCATsociety/SOEM/tree/342ca8632c3a495ea9700cc2ea189ca20c12c3e2)).
If you like to use an other version you can set the environment
variable `EC_SOEM_PATH`.

## Credits

Most of this crate was done by [Matwey V. Kornilov](https://github.com/matwey).

## License

This crate is licensed under the
[GPLv2.0](https://opensource.org/licenses/GPL-2.0)
license.
