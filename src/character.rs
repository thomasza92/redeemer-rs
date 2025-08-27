use bevy::prelude::*;
use avian2d::{math::*, prelude::*};
use crate::physics::DynamicFall;
use crate::level::PassThroughOneWayPlatform;

#[derive(Component)]
pub struct Actor;

#[derive(Component)]
pub struct MovementSpeed(pub Scalar);

#[derive(Component)]
pub struct JumpImpulse(pub Scalar);

pub fn spawn_main_character(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>) {
    let actor_size = Vector::new(20.0, 20.0);
    let actor_mesh = meshes.add(Rectangle::from_size(actor_size.f32()));
    let actor_material = materials.add(Color::srgb(0.2, 0.7, 0.9));

    commands.spawn((
        Mesh2d(actor_mesh),
        MeshMaterial2d(actor_material),
        RigidBody::Dynamic,
        LockedAxes::ROTATION_LOCKED,
        Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        LinearDamping(2.0),
        Collider::circle(actor_size.x - 9.0),
        Actor,
        SpeculativeMargin(0.25),
        PassThroughOneWayPlatform::Never,
        MovementSpeed(250.0),
        JumpImpulse(800.0),
        DynamicFall {
        base_g: 1000.0,
        max_g:  2200.0,
        grow_k: 3.0,
        t_fall: 0.0,
    },
    ));
}