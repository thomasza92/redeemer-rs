mod level;
mod camera;
mod animations;
mod character;
mod class;
mod prelude;

use crate::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_egui::EguiPlugin;
use crate::animations::PlayerAnimationsPlugin;
use crate::character::PlayerPlugin;
use crate::level::{spawn_map, pass_through_one_way_platform, PlatformerCollisionHooks};
use crate::character::{Action, spawn_main_character};
use crate::camera::{spawn_follow_camera, camera_follow, spawn_streetlights};
use crate::class::ClassPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                title: String::from("redeemer"),
                        ..Default::default()
                    }),
                    ..default()
                }
                )
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
                PhysicsDebugPlugin::default(),
        ))
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(TiledPlugin::default())
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default()) 
        .add_plugins(SpritesheetAnimationPlugin)
        .add_plugins(PlayerAnimationsPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(ClassPlugin::new("assets/class_unknown.json")
                .spawn_debug_holder(false),
        )
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))
        .add_systems(Startup, (spawn_map, spawn_main_character, spawn_follow_camera, spawn_streetlights))
        .add_systems(FixedUpdate, (pass_through_one_way_platform, camera_follow))
        .run();
}