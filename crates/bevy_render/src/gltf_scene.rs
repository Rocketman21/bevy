use crate::{
    renderer::{BufferInfo, BufferUsage, RenderResourceContext, RenderResourceId},
    mesh::{VERTEX_BUFFER_ASSET_INDEX, Mesh, INDEX_BUFFER_ASSET_INDEX, Vertex},
    pipeline::{
        AsVertexBufferDescriptor, IndexFormat, RenderPipelines,
        VertexBufferDescriptor, VertexBufferDescriptors,
    },
};
use bevy_asset::{AssetEvent, Handle, Assets};
use bevy_app::{Events, EventReader};
use bevy_ecs::{Res, Local, ResMut, Query};
use std::collections::HashSet;

pub struct GltfScene {
    pub meshes: Vec<Mesh>,
}

impl GltfScene {
    pub fn new() -> Self {
        GltfScene { meshes: Vec::new() }
    }
}

fn remove_current_mesh_resources(
    render_resource_context: &dyn RenderResourceContext,
    handle: Handle<GltfScene>,
) {
    if let Some(RenderResourceId::Buffer(buffer)) =
        render_resource_context.get_asset_resource(handle, VERTEX_BUFFER_ASSET_INDEX)
    {
        render_resource_context.remove_buffer(buffer);
        render_resource_context.remove_asset_resource(handle, VERTEX_BUFFER_ASSET_INDEX);
    }
    if let Some(RenderResourceId::Buffer(buffer)) =
        render_resource_context.get_asset_resource(handle, INDEX_BUFFER_ASSET_INDEX)
    {
        render_resource_context.remove_buffer(buffer);
        render_resource_context.remove_asset_resource(handle, INDEX_BUFFER_ASSET_INDEX);
    }
}

#[derive(Default)]
pub struct GltfSceneResourceProviderState {
    gltf_scene_event_reader: EventReader<AssetEvent<GltfScene>>,
    vertex_buffer_descriptor: Option<&'static VertexBufferDescriptor>,
}

pub fn gttf_scene_resource_provider_system(
    mut state: Local<GltfSceneResourceProviderState>,
    render_resource_context: Res<Box<dyn RenderResourceContext>>,
    gltf_scenes: Res<Assets<GltfScene>>,
    mut vertex_buffer_descriptors: ResMut<VertexBufferDescriptors>,
    gltf_scene_events: Res<Events<AssetEvent<GltfScene>>>,
    mut query: Query<(&Handle<GltfScene>, &mut RenderPipelines)>,
) {
    let vertex_buffer_descriptor = match state.vertex_buffer_descriptor {
        Some(value) => value,
        None => {
            // TODO: allow pipelines to specialize on vertex_buffer_descriptor and index_format
            let vertex_buffer_descriptor = Vertex::as_vertex_buffer_descriptor();
            vertex_buffer_descriptors.set(vertex_buffer_descriptor.clone());
            state.vertex_buffer_descriptor = Some(vertex_buffer_descriptor);
            vertex_buffer_descriptor
        }
    };
    let mut changed_gtlf_scenes = HashSet::new();
    let render_resource_context = &**render_resource_context;
    for event in state.gltf_scene_event_reader.iter(&gltf_scene_events) {
        match event {
            AssetEvent::Created { handle } => {
                changed_gtlf_scenes.insert(*handle);
            }
            AssetEvent::Modified { handle } => {
                changed_gtlf_scenes.insert(*handle);
                remove_current_mesh_resources(render_resource_context, *handle);
            }
            AssetEvent::Removed { handle } => {
                remove_current_mesh_resources(render_resource_context, *handle);
                // if mesh was modified and removed in the same update, ignore the modification
                // events are ordered so future modification events are ok
                changed_gtlf_scenes.remove(handle);
            }
        }
    }

    for changed_gltf_scene_handle in changed_gtlf_scenes.iter() {
        if let Some(gltf_scene) = gltf_scenes.get(changed_gltf_scene_handle) {
            for mesh in gltf_scene.meshes.iter() {
                let vertex_bytes = mesh
                    .get_vertex_buffer_bytes(&vertex_buffer_descriptor)
                    .unwrap();
                // TODO: use a staging buffer here
                let vertex_buffer = render_resource_context.create_buffer_with_data(
                    BufferInfo {
                        buffer_usage: BufferUsage::VERTEX,
                        ..Default::default()
                    },
                    &vertex_bytes,
                );
    
                let index_bytes = mesh.get_index_buffer_bytes(IndexFormat::Uint16).unwrap();
                let index_buffer = render_resource_context.create_buffer_with_data(
                    BufferInfo {
                        buffer_usage: BufferUsage::INDEX,
                        ..Default::default()
                    },
                    &index_bytes,
                );
    
                render_resource_context.set_asset_resource(
                    *changed_gltf_scene_handle,
                    RenderResourceId::Buffer(vertex_buffer),
                    VERTEX_BUFFER_ASSET_INDEX,
                );
                render_resource_context.set_asset_resource(
                    *changed_gltf_scene_handle,
                    RenderResourceId::Buffer(index_buffer),
                    INDEX_BUFFER_ASSET_INDEX,
                );
            }
        }
    }

    // TODO: remove this once batches are pipeline specific and deprecate assigned_meshes draw target
    for (handle, mut render_pipelines) in &mut query.iter() {
        if let Some(gltf_scene) = gltf_scenes.get(&handle) {
            for mesh in gltf_scene.meshes.iter() {
                for render_pipeline in render_pipelines.pipelines.iter_mut() {
                    render_pipeline.specialization.primitive_topology = mesh.primitive_topology;
                }
            }
        }

        if let Some(RenderResourceId::Buffer(vertex_buffer)) =
            render_resource_context.get_asset_resource(*handle, VERTEX_BUFFER_ASSET_INDEX)
        {
            render_pipelines.bindings.set_vertex_buffer(
                "Vertex",
                vertex_buffer,
                render_resource_context
                    .get_asset_resource(*handle, INDEX_BUFFER_ASSET_INDEX)
                    .and_then(|r| {
                        if let RenderResourceId::Buffer(buffer) = r {
                            Some(buffer)
                        } else {
                            None
                        }
                    }),
            );
        }
    }
}
