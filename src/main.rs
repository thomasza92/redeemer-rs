mod animations;
mod camera;
mod character;
mod class;
mod enemy;
mod filmic_post;
mod gameflow;
mod halation_post;
mod hud;
mod level;
mod prelude;
mod raycasts;

use crate::MonitorSelection::*;
use crate::animations::PlayerAnimationsPlugin;
use crate::camera::{
    camera_follow, despawn_main_camera, despawn_menu_camera, spawn_follow_camera, spawn_menu_camera,
};
use crate::character::{Action, PlayerPlugin, spawn_main_character};
use crate::class::ClassPlugin;
use crate::enemy::{EnemyPlugin, spawn_enemy};
use crate::filmic_post::FilmicControls;
use crate::filmic_post::FilmicPostProcessPlugin;
use crate::filmic_post::FilmicSettings;
use crate::filmic_post::sync_filmic_controls;
use crate::gameflow::{GameFlowPlugin, GameState, despawn_gameplay};
use crate::halation_post::HalationPostProcessPlugin;
use crate::hud::HudPlugin;
use crate::level::{PlatformerCollisionHooks, pass_through_one_way_platform, spawn_map};
use crate::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_window::PresentMode;
use bevy_window::WindowMode;
use vleue_kinetoscope::AnimatedImagePlugin;

#[derive(Resource)]
struct WorldLoaded;

fn world_not_loaded(flag: Option<Res<WorldLoaded>>) -> bool {
    flag.is_none()
}

fn mark_world_loaded(mut commands: Commands) {
    commands.insert_resource(WorldLoaded);
}

fn clear_world_loaded(mut commands: Commands) {
    commands.remove_resource::<WorldLoaded>();
}

fn spawn_enemies(mut commands: Commands) {
    spawn_enemy(&mut commands, Vec2::new(200.0, 0.0), 160.0, 260.0);
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        mode: WindowMode::BorderlessFullscreen(Primary),
                        title: String::from("redeemer"),
                        present_mode: PresentMode::AutoNoVsync,
                        ..Default::default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    watch_for_changes_override: Some(true),
                    file_path: "../redeemer-rs/assets/".to_string(),
                    ..Default::default()
                })
                .set(ImagePlugin::default_nearest()),
            InputManagerPlugin::<Action>::default(),
            StateMachinePlugin::default(),
            PhysicsPlugins::default()
                .with_length_unit(2.0)
                .with_collision_hooks::<PlatformerCollisionHooks>(),
            Light2dPlugin,
            //            PhysicsDebugPlugin::default(),
        ))
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(SpritesheetAnimationPlugin)
        .add_plugins(PlayerAnimationsPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(EnemyPlugin)
        .add_plugins(ClassPlugin::new("assets/class_unknown.json").spawn_debug_holder(false))
        .add_plugins(HudPlugin)
        .add_plugins(AnimatedImagePlugin)
        .add_plugins(GameFlowPlugin)
        .add_plugins(TiledPlugin::default())
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default())
        .add_plugins(HalationPostProcessPlugin)
        .add_plugins(FilmicPostProcessPlugin)
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .register_type::<FilmicSettings>()
        .register_type::<FilmicControls>()
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))
        .add_systems(Startup, spawn_menu_camera)
        .add_systems(
            OnEnter(GameState::InGame),
            (
                despawn_menu_camera,
                (
                    spawn_map,
                    spawn_main_character,
                    spawn_follow_camera,
                    spawn_enemies,
                )
                    .run_if(world_not_loaded),
                mark_world_loaded.run_if(world_not_loaded),
            )
                .chain(),
        )
        .add_systems(
            OnEnter(GameState::MainMenu),
            (
                despawn_gameplay,
                despawn_main_camera,
                clear_world_loaded,
                spawn_menu_camera,
            ),
        )
        .add_systems(
            OnEnter(GameState::GameOver),
            (despawn_gameplay, clear_world_loaded),
        )
        .add_systems(
            FixedUpdate,
            (
                pass_through_one_way_platform,
                camera_follow,
                sync_filmic_controls,
            )
                .run_if(in_state(GameState::InGame)),
        )
        .run();
}
