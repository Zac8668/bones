//! Custom WGSL shader asset and sprite material components.

use crate::prelude::*;

/// A WGSL shader asset loaded from a `.wgsl` file.
#[derive(HasSchema, Clone, Debug, Default)]
#[type_data(asset_loader("wgsl", ShaderAssetLoader))]
pub struct Shader {
    /// The WGSL shader source code.
    pub source: String,
    /// Optional disk path to the shader file for manual `F5` hot-reloading.
    pub path: Option<std::path::PathBuf>,
}

/// Implements [`AssetLoader`] for `.wgsl` shader files.
pub struct ShaderAssetLoader;
impl AssetLoader for ShaderAssetLoader {
    fn load(&self, ctx: AssetLoadCtx, bytes: &[u8]) -> BoxedFuture<anyhow::Result<SchemaBox>> {
        let bytes = bytes.to_vec();
        let path = ctx
            .asset_server
            .io
            .resolve_path(ctx.loc.as_ref())
            .unwrap_or_else(|| ctx.loc.path.clone());
        Box::pin(async move {
            let source = String::from_utf8(bytes)?;
            Ok(SchemaBox::new(Shader {
                source,
                path: Some(path),
            }))
        })
    }
}

/// Component attaching a custom [`Shader`] and per-sprite parameters to a sprite entity.
#[derive(HasSchema, Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct SpriteMaterial {
    /// Handle to the custom [`Shader`] asset.
    pub shader: Handle<Shader>,
    /// First custom parameter vector passed to the shader (`shader_params0`).
    pub params0: Vec4,
    /// Second custom parameter vector passed to the shader (`shader_params1`).
    pub params1: Vec4,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::{block_on, yield_now};
    use std::path::Path;

    fn load_single_shader(asset_server: &AssetServer, path: &str) -> Handle<Shader> {
        bevy_tasks::IoTaskPool::init(bevy_tasks::TaskPool::default);
        let handle = asset_server.load_asset(AssetLocRef::from((Path::new(path), None)));
        block_on(async {
            while asset_server.load_progress.loaded() + asset_server.load_progress.errored()
                < asset_server.load_progress.to_load()
            {
                yield_now().await;
            }
        });
        handle.typed()
    }

    #[test]
    fn shader_and_material_schemas_register_and_load_wgsl() {
        Shader::register_schema();
        SpriteMaterial::register_schema();

        let wgsl_src = "@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";
        let asset_server = AssetServer::new(
            DummyIo::new([("/shaders/test.wgsl", wgsl_src.as_bytes().to_vec())]),
            Version::new(0, 1, 0),
        );
        let handle = load_single_shader(&asset_server, "/shaders/test.wgsl");
        let shader = asset_server.get(handle);
        assert_eq!(shader.source, wgsl_src);
        assert_eq!(
            shader.path.as_deref(),
            Some(Path::new("/shaders/test.wgsl"))
        );
    }

    #[test]
    fn shader_loader_resolves_disk_path_for_file_io_and_rejects_invalid_utf8() {
        Shader::register_schema();

        let temp_root =
            std::env::temp_dir().join(format!("bones_shader_loader_test_{}", std::process::id()));
        let core_dir = temp_root.join("assets");
        let packs_dir = temp_root.join("packs");
        let shader_dir = core_dir.join("shaders");
        std::fs::create_dir_all(&shader_dir).unwrap();
        let disk_path = shader_dir.join("glow.wgsl");
        std::fs::write(&disk_path, "fn f() {}").unwrap();
        std::fs::write(shader_dir.join("bad.wgsl"), [0xff, 0xfe]).unwrap();

        let asset_server = AssetServer::new(
            FileAssetIo::new(&core_dir, &packs_dir),
            Version::new(0, 1, 0),
        );
        let handle = load_single_shader(&asset_server, "/shaders/glow.wgsl");
        let shader = asset_server.get(handle);
        assert_eq!(shader.source, "fn f() {}");
        assert_eq!(shader.path.as_deref(), Some(disk_path.as_path()));

        let bad_handle = load_single_shader(&asset_server, "/shaders/bad.wgsl");
        assert!(asset_server.try_get(bad_handle).is_none());
        assert_eq!(asset_server.load_progress.errored(), 1);

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
