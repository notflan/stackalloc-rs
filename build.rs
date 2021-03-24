
extern crate rustc_version;
use rustc_version::{version, version_meta, Channel};

fn build_tramp()
{
    let mut builder = cc::Build::new();
    // --std=c99 -W -Wall -Werror -pedantic -O3 -flto
    builder.flag("--std=c99")
	.flag("-W")
	.flag("-Wall")
	.flag_if_supported("-Wextra")
	.flag("-Werror")
	.flag("-pedantic")
	.opt_level(3)
	.flag_if_supported("-flto")

 	// Not sure if we want these two. We can check the codegen later.
	.pic(false)
	.use_plt(false)
	
	.file("alloca_trampoline_.c")
	.compile("calloca_trampoline");
}

fn main() {
    // Assert we haven't travelled back in time
    assert!(version().unwrap().major >= 1);

    // Set cfg flags depending on release channel
    match version_meta().unwrap().channel {
        Channel::Stable => {
            println!("cargo:rustc-cfg=stable");
        }
        Channel::Beta => {
            println!("cargo:rustc-cfg=beta");
        }
        Channel::Nightly => {
            println!("cargo:rustc-cfg=nightly");
        }
        Channel::Dev => {
            println!("cargo:rustc-cfg=dev");
        }
    }

    build_tramp();
}
