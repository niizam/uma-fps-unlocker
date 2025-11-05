#[cfg(windows)]
fn main() {
    let manifest = r#"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>
<assembly manifestVersion=\"1.0\" xmlns=\"urn:schemas-microsoft-com:asm.v1\">
  <assemblyIdentity version=\"1.0.0.0\" processorArchitecture=\"*\" name=\"Uma-fps-unlocker\" type=\"win32\"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type=\"win32\"
        name=\"Microsoft.Windows.Common-Controls\"
        version=\"6.0.0.0\"
        processorArchitecture=\"*\"
        publicKeyToken=\"6595b64144ccf1df\"
        language=\"*\"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;

    let mut res = winres::WindowsResource::new();
    res.set_manifest(manifest);
    if let Err(e) = res.compile() {
        println!("cargo:warning=winres failed: {}", e);
    }
}

#[cfg(not(windows))]
fn main() {}

