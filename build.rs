fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set("FileDescription", "HTTP Discovery Service");
        res.set("ProductName", "HTTP Discovery Service");
        res.set("CompanyName", "spacedock-zero");
        res.set("OriginalFilename", "http-discovery-service.exe");
        res.set("LegalCopyright", "Copyright (c) 2026 spacedock-zero");
        // res.set_icon("icon.ico");
        res.compile().unwrap();
    }
}
