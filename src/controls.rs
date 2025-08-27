use bevy::prelude::*;
use avian2d::{math::*, prelude::*};
use crate::character::*;

pub fn setup_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut actors: Query<(&mut LinearVelocity, &MovementSpeed, &JumpImpulse), With<Actor>>,
) {
    for (mut linear_velocity, movement_speed, jump_impulse) in &mut actors {
        // Naive grounded check (matches your jump logic)
        let grounded = linear_velocity.y.abs() < 0.01;

        // Input
        let left  = keyboard_input.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]);
        let right = keyboard_input.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]);
        let horizontal = right as i8 - left as i8;

        // Sprint (Shift)
        let sprinting = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let sprint_mul: Scalar = if sprinting { 1.5 } else { 1.0 };
        linear_velocity.x = horizontal as Scalar * movement_speed.0 * sprint_mul;

        // Jump only when grounded (your existing logic)
        if grounded
            && keyboard_input.just_pressed(KeyCode::Space)
        {
            linear_velocity.y = jump_impulse.0;
        }
    }
}