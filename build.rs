// Puts MessageCraft's icon and version information into MessageCraft.exe on Windows.
fn main() {
    println!("cargo:rerun-if-changed=assets/messagecraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/messagecraft.ico")
            .set("ProductName", "MessageCraft")
            .set("FileDescription", "MessageCraft")
            .set("CompanyName", "MessageCraft contributors")
            .set("LegalCopyright", "MessageCraft contributors. MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            println!("cargo:warning=couldn't add the icon to MessageCraft.exe: {e}");
        }
    }
}
