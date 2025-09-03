mod level;
mod camera;
mod animations;
mod character;
mod class;
mod prelude;
mod hud;
mod gameflow;

use crate::prelude::*;
use bevy::prelude::in_state;
//use bevy_inspector_egui::quick::WorldInspectorPlugin;
//use bevy_egui::EguiPlugin;
use crate::animations::PlayerAnimationsPlugin;
use crate::character::PlayerPlugin;
use crate::level::{spawn_map, pass_through_one_way_platform, PlatformerCollisionHooks};
use crate::character::{Action, spawn_main_character};
use crate::camera::{spawn_follow_camera, camera_follow, spawn_streetlights};
use crate::class::ClassPlugin;
use crate::hud::HudPlugin;
use crate::gameflow::{GameFlowPlugin, GameState, despawn_gameplay};

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
//        .add_plugins(EguiPlugin::default())
//        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(TiledPlugin::default())
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default())
        .add_plugins(SpritesheetAnimationPlugin)
        .add_plugins(PlayerAnimationsPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(
            ClassPlugin::new("assets/class_unknown.json")
                .spawn_debug_holder(false),
        )
        .add_plugins(HudPlugin)
        .add_plugins(GameFlowPlugin)

        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))

        // 1) Spawn a simple camera at startup so menus/UI render.
        .add_systems(Startup, spawn_menu_camera)

        // 2) When we enter gameplay, FIRST remove the menu camera, THEN spawn the world.
        .add_systems(
            OnEnter(GameState::InGame),
            (
                despawn_menu_camera,                              // <-- remove UI-only camera
                (spawn_map, spawn_main_character,                 // then build the world
                 spawn_follow_camera, spawn_streetlights)
            ).chain()
        )

        // 3) When leaving gameplay (to MainMenu/GameOver), clean the world and
        //    respawn the UI-only camera so menus are visible again.
        .add_systems(OnExit(GameState::InGame), (despawn_gameplay, spawn_menu_camera))

        // Only tick gameplay systems during InGame.
        .add_systems(
            FixedUpdate,
            (pass_through_one_way_platform, camera_follow)
                .run_if(in_state(GameState::InGame))
        )
        .run();
}

#[derive(Component)]
struct MenuCamera;

fn spawn_menu_camera(mut commands: Commands, q_existing: Query<(), With<MenuCamera>>) {
    if q_existing.is_empty() {
        commands.spawn((Camera2d, MenuCamera));
    }
}

fn despawn_menu_camera(mut commands: Commands, q: Query<Entity, With<MenuCamera>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}