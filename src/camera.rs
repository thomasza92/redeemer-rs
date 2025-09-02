use bevy::prelude::*;
use crate::character::Actor;
use bevy_light_2d::prelude::*;

#[derive(Component)]
pub struct MainCamera;

pub fn spawn_follow_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 0.25;
    commands.spawn((
        MainCamera,
        Camera2d,
        Projection::Orthographic(projection),
        Light2d {
            ambient_light: AmbientLight2d {
                brightness: 0.1,
                ..default()
            },
        },
    ));
}

pub fn spawn_streetlights(mut commands: Commands) {
    commands.spawn((
        PointLight2d {
            color: Color::WHITE,
            radius: 100.0,
            intensity: 2.0,
            falloff: 0.5,
            cast_shadows: true,
            ..default()
        },
        Transform::from_xyz(116., 12., 1.),
    ));
    commands.spawn((
        PointLight2d {
            color: Color::WHITE,
            radius: 100.0,
            intensity: 2.0,
            falloff: 0.5,
            cast_shadows: true,
            ..default()
        },
        Transform::from_xyz(693., 12., 1.),
    ));
}

pub fn camera_follow(
    time: Res<Time>,
    player_q: Query<&GlobalTransform, With<Actor>>,
    mut cam_q: Query<&mut bevy::prelude::Transform, (With<MainCamera>, Without<Actor>)>,
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