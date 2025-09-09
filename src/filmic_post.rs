use bevy::{
    core_pipeline::{
        core_2d::graph::{Core2d, Node2d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    prelude::*,
    reflect::Reflect,
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
use bevy_inspector_egui::InspectorOptions;
use bevy_inspector_egui::prelude::ReflectInspectorOptions;

const SHADER_ASSET_PATH: &str = "shaders/filmic_post.wgsl";

#[derive(Component, Reflect, InspectorOptions)]
#[reflect(Component, InspectorOptions)]
pub struct FilmicControls {
    #[inspector(min=-3.0, max=3.0, speed=0.02)]
    pub exposure_ev: f32,

    #[inspector(min = 0.0, max = 1.0, speed = 0.01)]
    pub vignette_strength: f32,

    #[inspector(min = 0.0, max = 0.6, speed = 0.01)]
    pub shadow_crush: f32,

    #[inspector(min = 0.0, max = 1.2, speed = 0.01)]
    pub split_tone_strength: f32,

    #[inspector(min = 0.0, max = 4.0, speed = 0.02)]
    pub ca_amount_px: f32,

    #[inspector(min = 0.2, max = 3.0, speed = 0.01)]
    pub ca_falloff: f32,

    #[inspector(min = 0.0, max = 1.2, speed = 0.01)]
    pub curve_strength: f32,

    #[inspector(min = 0.0, max = 1.2, speed = 0.01)]
    pub stock_strength: f32,
}

impl Default for FilmicControls {
    fn default() -> Self {
        Self {
            exposure_ev: 0.15,
            vignette_strength: 0.22,
            shadow_crush: 0.03,
            split_tone_strength: 0.84,
            ca_amount_px: 2.00,
            ca_falloff: 1.48,
            curve_strength: 0.08,
            stock_strength: 0.18,
        }
    }
}

#[derive(Component, Clone, Copy, Default, ExtractComponent, ShaderType, Reflect)]
pub struct FilmicSettings {
    pub exposure_ev: f32,
    pub vignette_strength: f32,
    pub shadow_crush: f32,
    pub split_tone_strength: f32,
    pub ca_amount_px: f32,
    pub ca_falloff: f32,
    pub curve_strength: f32,
    pub stock_strength: f32,
}

impl FilmicSettings {
    pub fn default() -> Self {
        Self {
            exposure_ev: 0.15,
            vignette_strength: 0.22,
            shadow_crush: 0.03,
            split_tone_strength: 0.84,
            ca_amount_px: 2.00,
            ca_falloff: 1.48,
            curve_strength: 0.08,
            stock_strength: 0.18,
        }
    }
}

pub struct FilmicPostProcessPlugin;

impl Plugin for FilmicPostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<FilmicSettings>::default(),
            UniformComponentPlugin::<FilmicSettings>::default(),
        ));

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_render_graph_node::<ViewNodeRunner<FilmicNode>>(Core2d, FilmicLabel)
                .add_render_graph_edges(
                    Core2d,
                    (
                        Node2d::Tonemapping,
                        FilmicLabel,
                        Node2d::EndMainPassPostProcessing,
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<FilmicPipeline>();
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct FilmicLabel;

#[derive(Resource)]
struct FilmicPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for FilmicPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "filmic_post_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<FilmicSettings>(true),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let shader: Handle<Shader> = world.resource::<AssetServer>().load(SHADER_ASSET_PATH);

        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("filmic_post_pipeline".into()),
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
struct FilmicNode;

impl ViewNode for FilmicNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static FilmicSettings,
        &'static DynamicUniformIndex<FilmicSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _cpu_settings, dyn_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipe = world.resource::<FilmicPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(gpu_pipeline) = cache.get_render_pipeline(pipe.pipeline_id) else {
            return Ok(());
        };

        let settings_uni = world.resource::<ComponentUniforms<FilmicSettings>>();
        let Some(settings_binding) = settings_uni.uniforms().binding() else {
            return Ok(());
        };

        let post = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "filmic_post_bind_group",
            &pipe.layout,
            &BindGroupEntries::sequential((post.source, &pipe.sampler, settings_binding.clone())),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("filmic_post_pass"),
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

pub fn sync_filmic_controls(mut q: Query<(&FilmicControls, &mut FilmicSettings)>) {
    for (ui, mut s) in &mut q {
        s.exposure_ev = ui.exposure_ev;
        s.vignette_strength = ui.vignette_strength;
        s.shadow_crush = ui.shadow_crush;
        s.split_tone_strength = ui.split_tone_strength;
        s.ca_amount_px = ui.ca_amount_px;
        s.ca_falloff = ui.ca_falloff;
        s.curve_strength = ui.curve_strength;
        s.stock_strength = ui.stock_strength;
    }
}
