use avian2d::{math::*, prelude::*};
use bevy::prelude::*;
use crate::character::Actor;

#[derive(Component)]
pub struct DynamicFall {
    pub base_g: Scalar,
    pub max_g:  Scalar,
    pub grow_k: Scalar,
    pub t_fall: Scalar,
}

pub fn dynamic_fall_gravity(
    time: Res<Time<Fixed>>,
    mut q: Query<(&mut LinearVelocity, &mut DynamicFall), With<Actor>>,
) {
    let dt = time.delta_secs();
    for (mut vel, mut dynfall) in &mut q {
        if vel.y < 0.0 {

            // accumulate time while falling
            dynfall.t_fall += dt;

            // g(t) = base + (max-base)*(1 - e^{-k t})
            let g_t = dynfall.base_g
                + (dynfall.max_g - dynfall.base_g) * (1.0 - (-dynfall.grow_k * dynfall.t_fall).exp());

            // apply only the *extra* beyond base_g (global Gravity already applies base_g)
            let extra = g_t - dynfall.base_g;

            // negative is downward
            vel.y -= extra * dt;
        } else {

            // not falling: reset timer so next fall starts fresh
            dynfall.t_fall = 0.0;
        }
    }
}