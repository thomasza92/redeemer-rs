use bevy::prelude::*;
use crate::character::Actor;

#[derive(Component)]
pub struct MainCamera;

pub fn spawn_follow_camera(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));
}

pub fn camera_follow(
    time: Res<Time>,
    player_q: Query<&GlobalTransform, With<Actor>>,
    mut cam_q: Query<&mut bevy::prelude::Transform, (With<MainCamera>, Without<Actor>)>,
) {
    let Ok(player_gt) = player_q.single() else { return; };
    let Ok(mut cam_tf)  = cam_q.single_mut() else { return; };

    // Target the player's XY
    let target_xy  = player_gt.translation().truncate();
    let current_xy = cam_tf.translation.truncate();

    // Smooth follow (raise 10.0 to make it snappier)
    let t = 1.0 - (-10.0 * time.delta_secs()).exp();
    let new_xy = current_xy.lerp(target_xy, t);

    // Write just the XY; keep Z as-is
    cam_tf.translation.x = new_xy.x;
    cam_tf.translation.y = new_xy.y;
}