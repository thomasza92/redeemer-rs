use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, Clone, Eq, Hash, PartialEq, Reflect, Debug)]
pub enum Action {
    #[actionlike(Axis)]
    Move,
    Jump,
    Attack,
    Sprint,
}

#[derive(Clone, Copy, Component, Reflect)]
#[component(storage = "SparseSet")]
pub enum Grounded {
    Left = -1,
    Idle = 0,
    Right = 1,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct Falling {
    pub velocity: f32,
    pub vel_x: f32,
}

pub const GRAVITY: f32 = -980.;
pub const PLAYER_SPEED: f32 = 150.;
pub const SPRINT_MULTIPLIER: f32 = 1.75;
pub const JUMP_VELOCITY: f32 = 420.;

pub fn grounded(In(entity): In<Entity>, fallings: Query<(&Transform, &Falling)>) -> bool {
    let (transform, falling) = fallings.get(entity).unwrap();
    transform.translation.y <= 0. && falling.velocity <= 0.
}

pub fn walk(
    mut q: Query<(&mut Transform, &Grounded, &ActionState<Action>)>,
    time: Res<Time>,
) {
    for (mut transform, grounded, actions) in &mut q {
        let sprinting = actions.pressed(&Action::Sprint);
        let speed = PLAYER_SPEED * if sprinting { SPRINT_MULTIPLIER } else { 1.0 };
        transform.translation.x += *grounded as i32 as f32 * time.delta_secs() * speed;
    }
}

pub fn fall(mut q: Query<(&mut Transform, &mut Falling)>, time: Res<Time>) {
    for (mut transform, mut falling) in &mut q {
        let dt = time.delta_secs();
        falling.velocity += dt * GRAVITY;
        transform.translation.y += dt * falling.velocity;
        transform.translation.x += dt * falling.vel_x;
    }
}