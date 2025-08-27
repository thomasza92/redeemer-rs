use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_spritesheet_animation::prelude::*;
use leafwing_input_manager::prelude::*;
use seldom_state::prelude::*;
use avian2d::prelude::*;

use crate::controls::{Action, Grounded, Falling, grounded, JUMP_VELOCITY};
use crate::level::PassThroughOneWayPlatform;
use crate::animations::PlayerSpritesheet;

#[derive(Component)]
pub struct Actor;

/// Simple overlay state for attacks (separate from your locomotion states).
#[derive(Component, Clone, Copy, Default)]
pub struct Attacking;

pub fn spawn_main_character(
    mut commands: Commands,
    sheet: Res<PlayerSpritesheet>,
    library: Res<AnimationLibrary>,
) {
    // Animation ids (Option<AnimationId>)
    let idle_id   = library.animation_with_name("player:idle")
        .expect("missing animation: player:idle");
    // Optional lookups (used by on-enter systems below)
    let _walk_id  = library.animation_with_name("player:walk");
    let _run_id   = library.animation_with_name("player:run");
    let _jump_id  = library.animation_with_name("player:jump");
    let _fall_id  = library.animation_with_name("player:fall");
    let _atk_id   = library.animation_with_name("player:attack");

    // Sprite / atlas
    let mut sprite = Sprite::from_atlas_image(
        sheet.image.clone(),
        TextureAtlas { layout: sheet.layout.clone(), ..Default::default() },
    );
    sprite.anchor = Anchor::Custom(Vec2::new(0.0, -0.25));

    // Input map (adds Attack)
    let input = InputMap::default()
        .with_axis(Action::Move, VirtualAxis::horizontal_arrow_keys())
        .with_axis(Action::Move, GamepadControlAxis::new(GamepadAxis::LeftStickX))
        .with(Action::Jump, KeyCode::Space)
        .with(Action::Jump, GamepadButton::South)
        .with(Action::Attack, KeyCode::KeyJ)
        .with(Action::Attack, GamepadButton::West);

    commands.spawn((
        Actor,
        input,

        // === Initial locomotion state: keep your original type ===
        Grounded::Idle,

        // === State machine ===
        StateMachine::default()
            // Grounded -> Jump/Air
            .trans::<Grounded, _>(just_pressed(Action::Jump), Falling { velocity: JUMP_VELOCITY })

            // Air -> Grounded (use your existing predicate)
            .trans::<Falling, _>(grounded, Grounded::Idle)

            // Axis -> Idle/Left/Right (use your existing grounded enum)
            .trans_builder(value_unbounded(Action::Move), |t: Trans<Grounded, f32>| {
                let v = t.out;
                if v > 0.5 { Grounded::Right }
                else if v < -0.5 { Grounded::Left }
                else { Grounded::Idle }
            })

            // Start attack from ground or air
            .trans::<Grounded, _>(just_pressed(Action::Attack), Attacking::default())
            .trans::<Falling,  _>(just_pressed(Action::Attack), Attacking::default())

            // End attack on release (swap this for animation-finished if you want)
            .trans::<Attacking, _>(just_released(Action::Attack), Grounded::Idle)
        ,

        // === Render + start animation ===
        sprite,
        SpritesheetAnimation::from_id(idle_id),

        // === Physics (unchanged) ===
        RigidBody::Dynamic,
        LockedAxes::ROTATION_LOCKED,
        Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        LinearDamping(2.0),
        Collider::capsule(8.0, 26.0),
        SpeculativeMargin(0.1),
        PassThroughOneWayPlatform::Never,
    ));
}

/* ---------------- Animation swaps on state enter/change ---------------- */

pub fn anim_on_grounded_change(
    library: Res<AnimationLibrary>,
    mut q: Query<(&Grounded, &mut SpritesheetAnimation, &mut Transform), Changed<Grounded>>,
) {
    for (g, mut anim, mut tf) in &mut q {
        let (name, face_right) = match g {
            Grounded::Idle  => ("player:idle",  None),
            Grounded::Left  => ("player:walk",  Some(false)),
            Grounded::Right => ("player:walk",  Some(true)),
        };

        if let Some(id) = library.animation_with_name(name) {
            // Only switch clips if the target clip is different; this preserves progress.
            if anim.animation_id != id {
                anim.switch(id);
            }
            // Make sure it’s playing.
            anim.playing = true;
        }

        // Flip without touching the animation clip.
        if let Some(right) = face_right {
            if right { tf.scale.x = tf.scale.x.abs(); }
            else     { tf.scale.x = -tf.scale.x.abs(); }
        }
    }
}

pub fn anim_on_enter_falling(
    library: Res<AnimationLibrary>,
    mut q: Query<&mut SpritesheetAnimation, Added<Falling>>,
) {
    if let Some(id) = library
        .animation_with_name("player:jump")
        .or_else(|| library.animation_with_name("player:fall"))
    {
        for mut anim in &mut q {
            if anim.animation_id != id {
                anim.switch(id);
            }
            anim.playing = true;
        }
    }
}

pub fn anim_on_enter_attacking(
    library: Res<AnimationLibrary>,
    mut q: Query<&mut SpritesheetAnimation, Added<Attacking>>,
) {
    if let Some(id) = library.animation_with_name("player:attack") {
        for mut anim in &mut q {
            if anim.animation_id != id {
                anim.switch(id);
            }
            anim.playing = true;
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Update,
                (
                    anim_on_grounded_change,
                    anim_on_enter_falling,
                    anim_on_enter_attacking,
                ),
            );
    }
}