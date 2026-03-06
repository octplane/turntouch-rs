fn main() {
    #[cfg(target_os = "macos")]
    {
        let plist_path = std::path::Path::new("Info.plist")
            .canonicalize()
            .expect("Info.plist not found");
        println!("cargo::rerun-if-changed=Info.plist");
        println!("cargo::rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{}", plist_path.display());
    }
}
