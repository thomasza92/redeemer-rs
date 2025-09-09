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

/// WGSL file from my previous message
const SHADER_ASSET_PATH: &str = "shaders/halation_post.wgsl";

/// Matches the WGSL struct exactly (3x vec4)
#[derive(Component, Clone, Copy, Default, ExtractComponent, ShaderType)]
pub struct HalationSettings {
    pub p0: Vec4, // (strength, radius_px, threshold, knee)
    pub p1: Vec4, // (tint.r, tint.g, tint.b, red_boost)
    pub p2: Vec4, // (shadow_mul, _, _, _)
}

pub struct HalationPostProcessPlugin;

impl Plugin for HalationPostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<HalationSettings>::default(),
            UniformComponentPlugin::<HalationSettings>::default(),
        ));

        // Add a view node to the 2D graph
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_render_graph_node::<ViewNodeRunner<HalationNode>>(Core2d, HalationLabel)
                // run after tonemapping, before the end of post-processing
                .add_render_graph_edges(
                    Core2d,
                    (
                        Node2d::Tonemapping,
                        HalationLabel,
                        Node2d::EndMainPassPostProcessing,
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<HalationPipeline>();
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct HalationLabel; // pub so you can order it relative to your dither node

#[derive(Resource)]
struct HalationPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for HalationPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "halation_post_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<HalationSettings>(true),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let shader: Handle<Shader> = world.resource::<AssetServer>().load(SHADER_ASSET_PATH);

        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("halation_post_pipeline".into()),
                    layout: vec![layout.clone()],
                    vertex: fullscreen_shader_vertex_state(),
                    fragment: Some(FragmentState {
                        shader,
                        shader_defs: Default::default(), // required on newer Bevy
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
                    zero_initialize_workgroup_memory: true, // required on newer Bevy
                });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

#[derive(Default)]
struct HalationNode;

impl ViewNode for HalationNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static HalationSettings,
        &'static DynamicUniformIndex<HalationSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _cpu_settings, dyn_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipe = world.resource::<HalationPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(gpu_pipeline) = cache.get_render_pipeline(pipe.pipeline_id) else {
            return Ok(());
        };

        // grab the GPU uniform buffer for all HalationSettings
        let settings_uni = world.resource::<ComponentUniforms<HalationSettings>>();
        let Some(settings_binding) = settings_uni.uniforms().binding() else {
            return Ok(());
        };

        let post = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "halation_post_bind_group",
            &pipe.layout,
            &BindGroupEntries::sequential((post.source, &pipe.sampler, settings_binding.clone())),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("halation_post_pass"),
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
        pass.set_bind_group(0, &bind_group, &[dyn_index.index()]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
