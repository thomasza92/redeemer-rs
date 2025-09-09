use bevy::{
    core_pipeline::{
        core_2d::graph::{Core2d, Node2d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        RenderApp,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice},
        view::ViewTarget,
    },
};

/// Path to the WGSL file below
const SHADER_ASSET_PATH: &str = "shaders/dither_post.wgsl";

/// Tweakable settings you add to your Camera2d
#[derive(Component, Clone, Copy, Default, ExtractComponent, ShaderType)]
pub struct DitherSettings {
    /// Number of output levels per channel (min 2). Use 2 for 1-bit, 4 for GB-ish, etc.
    pub levels: u32,
    /// 1 = grayscale dither, 0 = color dither
    pub monochrome: u32,
}

pub struct DitherPostProcessPlugin;

impl Plugin for DitherPostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<DitherSettings>::default(),
            UniformComponentPlugin::<DitherSettings>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // run on Core2d graph (2D cameras)
            .add_render_graph_node::<ViewNodeRunner<DitherNode>>(Core2d, DitherLabel)
            // place between tonemapping and end of post-processing for 2D
            .add_render_graph_edges(
                Core2d,
                (
                    Node2d::Tonemapping,
                    DitherLabel,
                    Node2d::EndMainPassPostProcessing,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<DitherPipeline>();
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct DitherLabel;

#[derive(Resource)]
struct DitherPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for DitherPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = render_device.create_bind_group_layout(
            "dither_post_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }), // screen texture
                    sampler(SamplerBindingType::Filtering),                    // sampler
                    uniform_buffer::<DitherSettings>(true),                    // settings
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let shader = world.load_asset(SHADER_ASSET_PATH);
        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("dither_post_pipeline".into()),
                    layout: vec![layout.clone()],
                    vertex: fullscreen_shader_vertex_state(),
                    fragment: Some(FragmentState {
                        shader,
                        shader_defs: Default::default(),
                        entry_point: "fragment".into(),
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::bevy_default(),
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    push_constant_ranges: vec![],
                    zero_initialize_workgroup_memory: true,
                });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

#[derive(Default)]
struct DitherNode;

impl ViewNode for DitherNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static DitherSettings,
        &'static DynamicUniformIndex<DitherSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _settings_cpu, settings_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<DitherPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(gpu_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<DitherSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "dither_post_bind_group",
            &pipeline.layout,
            &BindGroupEntries::sequential((
                post.source,
                &pipeline.sampler,
                settings_binding.clone(),
            )),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("dither_post_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_render_pipeline(gpu_pipeline);
        pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
