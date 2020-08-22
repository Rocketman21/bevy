use bevy_render::{
    mesh::{Mesh, VertexAttribute},
    pipeline::PrimitiveTopology,
    gltf_scene::GltfScene,
};

use anyhow::Result;
use bevy_asset::AssetLoader;
use gltf::{buffer::Source, iter, mesh::Mode};
use std::{fs, io, path::Path};
use thiserror::Error;

/// Loads meshes from GLTF files into Mesh assets
///
/// NOTE: eventually this will loading into Scenes instead of Meshes
#[derive(Default)]
pub struct GltfLoader;

impl AssetLoader<GltfScene> for GltfLoader {
    fn from_bytes(&self, asset_path: &Path, bytes: Vec<u8>) -> Result<GltfScene> {
        let gltf_scene = load_gltf(asset_path, bytes)?;
        Ok(gltf_scene)
    }

    fn extensions(&self) -> &[&str] {
        static EXTENSIONS: &[&str] = &["gltf"];
        EXTENSIONS
    }
}

/// An error that occurs when loading a GLTF file
#[derive(Error, Debug)]
pub enum GltfError {
    #[error("Unsupported primitive mode.")]
    UnsupportedPrimitive { mode: Mode },
    #[error("Invalid GLTF file.")]
    Gltf(#[from] gltf::Error),
    #[error("Failed to load file.")]
    Io(#[from] io::Error),
    #[error("Binary buffers not supported yet.")]
    BinaryBuffersUnsupported,
    #[error("Failed to decode base64 mesh data.")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("Unsupported buffer format.")]
    BufferFormatUnsupported,
}

fn get_primitive_topology(mode: Mode) -> Result<PrimitiveTopology, GltfError> {
    match mode {
        Mode::Points => Ok(PrimitiveTopology::PointList),
        Mode::Lines => Ok(PrimitiveTopology::LineList),
        Mode::LineStrip => Ok(PrimitiveTopology::LineStrip),
        Mode::Triangles => Ok(PrimitiveTopology::TriangleList),
        Mode::TriangleStrip => Ok(PrimitiveTopology::TriangleStrip),
        mode => Err(GltfError::UnsupportedPrimitive { mode }),
    }
}

pub fn load_gltf(asset_path: &Path, bytes: Vec<u8>) -> Result<GltfScene, GltfError> {
    let gltf = gltf::Gltf::from_slice(&bytes)?;
    let buffer_data = load_buffers(gltf.buffers(), asset_path)?;
    let mut meshes = Vec::new();

    for scene in gltf.scenes() {
        for node in scene.nodes() {
            if let Ok(mesh) = load_node(&buffer_data, &node) {
                meshes.extend(mesh);
            }
        }
    }

    if meshes.len() == 0 {
        // TODO: remove this when full gltf support is added
        panic!("no mesh found!");
    }

    println!("Meshes {}", &meshes.len());

    Ok(GltfScene { meshes })
}

fn load_node(buffer_data: &[Vec<u8>], node: &gltf::Node) -> Result<Vec<Mesh>, GltfError> {
    let mut meshes = Vec::new();

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));
            let primitive_topology = get_primitive_topology(primitive.mode())?;
            let mut mesh = Mesh::new(primitive_topology);

            if let Some(vertex_attribute) = reader
                .read_positions()
                .map(|v| VertexAttribute::position(v.collect()))
            {
                mesh.attributes.push(vertex_attribute);
            }

            if let Some(vertex_attribute) = reader
                .read_normals()
                .map(|v| VertexAttribute::normal(v.collect()))
            {
                mesh.attributes.push(vertex_attribute);
            }

            if let Some(vertex_attribute) = reader
                .read_tex_coords(0)
                .map(|v| VertexAttribute::uv(v.into_f32().collect()))
            {
                mesh.attributes.push(vertex_attribute);
            }

            if let Some(indices) = reader.read_indices() {
                mesh.indices = Some(indices.into_u32().collect::<Vec<u32>>());
            };

            meshes.push(mesh);
        }
    }

    for child in node.children() {
        if let Ok(child_meshes) = load_node(buffer_data, &child) {
            meshes.extend(child_meshes);
        }
    }

    if meshes.len() == 0 {
        panic!("failed to find mesh");
    }

    Ok(meshes)
}

fn load_buffers(buffers: iter::Buffers, asset_path: &Path) -> Result<Vec<Vec<u8>>, GltfError> {
    const OCTET_STREAM_URI: &str = "data:application/octet-stream;base64,";

    let mut buffer_data = Vec::new();
    for buffer in buffers {
        match buffer.source() {
            Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    if uri.starts_with(OCTET_STREAM_URI) {
                        buffer_data.push(base64::decode(&uri[OCTET_STREAM_URI.len()..])?);
                    } else {
                        return Err(GltfError::BufferFormatUnsupported);
                    }
                } else {
                    let buffer_path = asset_path.parent().unwrap().join(uri);
                    let buffer_bytes = fs::read(buffer_path)?;
                    buffer_data.push(buffer_bytes);
                }
            }
            Source::Bin => return Err(GltfError::BinaryBuffersUnsupported),
        }
    }

    Ok(buffer_data)
}
