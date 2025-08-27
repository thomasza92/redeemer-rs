use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, Clone, Eq, Hash, PartialEq, Reflect, Debug)]
pub enum Action {
    #[actionlike(Axis)]
    Move,
    Jump,
    Attack,
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
}

pub const GRAVITY: f32 = -1000.;
pub const PLAYER_SPEED: f32 = 200.;
pub const JUMP_VELOCITY: f32 = 500.;

pub fn grounded(In(entity): In<Entity>, fallings: Query<(&Transform, &Falling)>) -> bool {
    let (transform, falling) = fallings.get(entity).unwrap();
    transform.translation.y <= 0. && falling.velocity <= 0.
}

pub fn walk(mut groundeds: Query<(&mut Transform, &Grounded)>, time: Res<Time>) {
    for (mut transform, grounded) in &mut groundeds {
        transform.translation.x += *grounded as i32 as f32 * time.delta_secs() * PLAYER_SPEED;
    }
}

pub fn fall(mut fallings: Query<(&mut Transform, &mut Falling)>, time: Res<Time>) {
    for (mut transform, mut falling) in &mut fallings {
        let dt = time.delta_secs();
        falling.velocity += dt * GRAVITY;
        transform.translation.y += dt * falling.velocity;
    }
}