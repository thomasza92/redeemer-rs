mod level;
mod character;
mod camera;
mod controls;
mod animations;

use avian2d::{math::*, prelude::*};
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use bevy_light_2d::prelude::*;
use crate::level::{spawn_map, pass_through_one_way_platform, PlatformerCollisionHooks};
use crate::character::spawn_main_character;
use crate::camera::{spawn_follow_camera, camera_follow, spawn_streetlights};
use leafwing_input_manager::prelude::*;
use seldom_state::prelude::*;
use crate::controls::{Action, walk, fall};
use bevy_spritesheet_animation::prelude::*;
use crate::animations::PlayerAnimationsPlugin;
use crate::character::PlayerPlugin;

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
        .add_plugins(TiledPlugin::default())
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default()) 
        .add_plugins(SpritesheetAnimationPlugin)
        .add_plugins(PlayerAnimationsPlugin)
        .add_plugins(PlayerPlugin)
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))
        .add_systems(Startup, (spawn_map, spawn_main_character, spawn_follow_camera, spawn_streetlights))
        .add_systems(FixedUpdate, (walk, fall, pass_through_one_way_platform, camera_follow))
        .run();
}