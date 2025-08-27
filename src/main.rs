mod level;
mod character;
mod camera;
mod physics;
mod controls;


use avian2d::{math::*, prelude::*};
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use crate::level::{spawn_map, pass_through_one_way_platform, PlatformerCollisionHooks};
use crate::character::spawn_main_character;
use crate::camera::{spawn_follow_camera, camera_follow};
use crate::controls::setup_movement;
use crate::physics::dynamic_fall_gravity;

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
                })
                .set(ImagePlugin::default_nearest()),
                PhysicsPlugins::default()
                .with_length_unit(20.0)
                .with_collision_hooks::<PlatformerCollisionHooks>(),
        ))
        .add_plugins(TiledPlugin::default())
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default()) 
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))
        .add_systems(Startup, (spawn_map, spawn_main_character, spawn_follow_camera))
        .add_systems(FixedUpdate, (dynamic_fall_gravity, setup_movement, pass_through_one_way_platform, camera_follow))
        .run();
}