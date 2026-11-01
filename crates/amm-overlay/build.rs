use std::error::Error;
use std::ffi::CString;
use std::path::{Path, PathBuf};

use windows::Win32::Graphics::Direct3D::Fxc::{D3DCOMPILE_OPTIMIZATION_LEVEL3, D3DCompile};
use windows::Win32::Graphics::Direct3D::{ID3DBlob, ID3DInclude};
use windows::core::PCSTR;

/// Compiles `src/shaders.hlsl` into one bytecode file per entry point in `OUT_DIR`, so the game process never
/// loads `d3dcompiler_47.dll`. The layer renderer embeds the files with `include_bytes!`.
struct ShaderCompiler {
    source: String,
    out_dir: PathBuf,
}

impl ShaderCompiler {
    const SOURCE_PATH: &'static str = "src/shaders.hlsl";
    const SOURCE_NAME: &'static str = "amm_layers";
    const ENTRIES: [(&'static str, &'static str); 3] =
        [("vs_main", "vs_4_0"), ("ps_main", "ps_4_0"), ("ps_picture", "ps_4_0")];

    fn new(out_dir: PathBuf) -> Result<Self, Box<dyn Error>> {
        let source = std::fs::read_to_string(Self::SOURCE_PATH)?;
        Ok(Self { source, out_dir })
    }

    fn run(&self) -> Result<(), Box<dyn Error>> {
        for (entry, target) in Self::ENTRIES {
            let code = self.compile(entry, target)?;
            std::fs::write(self.out_dir.join(Path::new(entry).with_extension("cso")), code)?;
        }
        Ok(())
    }

    fn compile(&self, entry: &str, target: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let name = CString::new(Self::SOURCE_NAME)?;
        let entry_name = CString::new(entry)?;
        let target_name = CString::new(target)?;
        let mut code: Option<ID3DBlob> = None;
        let mut errors: Option<ID3DBlob> = None;
        // SAFETY: the source, name, entry and target buffers outlive the call, and both out pointers are writable
        let result = unsafe {
            D3DCompile(
                self.source.as_ptr().cast(),
                self.source.len(),
                PCSTR::from_raw(name.as_ptr().cast()),
                None,
                None::<&ID3DInclude>,
                PCSTR::from_raw(entry_name.as_ptr().cast()),
                PCSTR::from_raw(target_name.as_ptr().cast()),
                D3DCOMPILE_OPTIMIZATION_LEVEL3,
                0,
                &mut code,
                Some(&mut errors),
            )
        };
        match (result, code) {
            (Ok(()), Some(code)) => Ok(Self::blob_bytes(&code).to_vec()),
            _ => {
                let output = errors.map(|blob| String::from_utf8_lossy(Self::blob_bytes(&blob)).into_owned());
                Err(format!("{entry} did not compile: {}", output.unwrap_or_default()).into())
            }
        }
    }

    fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
        // SAFETY: a blob owns GetBufferSize bytes at GetBufferPointer for as long as it lives
        unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize()) }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed={}", ShaderCompiler::SOURCE_PATH);
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    ShaderCompiler::new(out_dir)?.run()
}
