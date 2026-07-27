fn main() {
    println!("cargo:rerun-if-changed=vendor/harfbuzz/src/harfbuzz-subset.cc");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("vendor/harfbuzz/src/harfbuzz-subset.cc")
        .define("HAVE_PTHREAD", None)
        .warnings(false)
        .compile("harfbuzz-subset");
}
