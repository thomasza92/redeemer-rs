use crate::prelude::*;
use crate::character::Player;
use crate::gameflow::GameplayRoot;
use bevy_light_2d::light::SpotLight2d;
use bevy_egui::PrimaryEguiContext;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct MenuCamera;

pub fn spawn_follow_camera(
    mut commands: Commands,
    existing: Query<(), With<MainCamera>>,
) {
    if existing.is_empty() {
        let mut projection = OrthographicProjection::default_2d();
        projection.scale = 0.25;
        commands.spawn((
            MainCamera,
            Camera2d,
            TiledParallaxCamera,
            PrimaryEguiContext,
            Projection::Orthographic(projection),
            Light2d {
                ambient_light: AmbientLight2d { brightness: 0.1, ..default() },
            },
        ));
    }
}

pub fn spawn_streetlights(mut commands: Commands) {
    commands.spawn((
        GameplayRoot,
        SpotLight2d {
        color: Srgba::hex("#FABD8A").unwrap().into(),
        intensity: 2.5,
        radius: 160.0,
        falloff: 2.5,
        direction: -90.,
        inner_angle: -180.,
        outer_angle: -90.,
        cast_shadows: true,
        ..default()
    },
        Transform::from_xyz(117., 332., 1.),
    ));
    commands.spawn((
        GameplayRoot,
        SpotLight2d {
        color: Srgba::hex("#FABD8A").unwrap().into(),
        intensity: 2.5,
        radius: 160.0,
        falloff: 2.5,
        direction: -90.,
        inner_angle: -180.,
        outer_angle: -90.,
        cast_shadows: true,
        ..default()
    },
        Transform::from_xyz(693., 332., 1.),
    ));
}

pub fn camera_follow(
    time: Res<Time>,
    player_q: Query<&GlobalTransform, With<Player>>,
    mut cam_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok(player_gt) = player_q.single() else { return; };
    let Ok(mut cam_tf)  = cam_q.single_mut() else { return; };
    let cam_adjust = Vec2::new(0., 3.);
    let target_xy  = player_gt.translation().truncate() + cam_adjust;
    let current_xy = cam_tf.translation.truncate() + cam_adjust;
    let t = 1.0 - (-10.0 * time.delta_secs()).exp();
    let new_xy = current_xy.lerp(target_xy, t);
    cam_tf.translation.x = new_xy.x;
    cam_tf.translation.y = new_xy.y;
}

pub fn spawn_menu_camera(mut commands: Commands, q_existing: Query<(), With<MenuCamera>>) {
    if q_existing.is_empty() {
        commands.spawn((Camera2d, MenuCamera));
    }
}
pub fn despawn_menu_camera(mut commands: Commands, q: Query<Entity, With<MenuCamera>>) {
    for e in &q { commands.entity(e).despawn(); }
}
pub fn despawn_main_camera(mut commands: Commands, q: Query<Entity, With<MainCamera>>) {
    for e in &q { commands.entity(e).despawn(); }
}