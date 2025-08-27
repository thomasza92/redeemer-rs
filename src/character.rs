use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_spritesheet_animation::prelude::*;
use leafwing_input_manager::prelude::*;
use seldom_state::prelude::*;
use avian2d::prelude::*;

use crate::controls::{
    Action, Grounded, Falling, grounded, JUMP_VELOCITY, PLAYER_SPEED, SPRINT_MULTIPLIER,
};
use crate::level::PassThroughOneWayPlatform;
use crate::animations::PlayerSpritesheet;

#[derive(Component)]
pub struct Actor;

#[derive(Component, Clone, Copy, Default)]
pub struct Attacking;

#[derive(Component, Clone, Copy)]
pub struct AnimClips {
    pub idle: AnimationId,
    pub walk: Option<AnimationId>,
    pub run: Option<AnimationId>,
    pub jump: Option<AnimationId>,
    pub fall: Option<AnimationId>,
    pub attack: Option<AnimationId>,
}

pub fn spawn_main_character(
    mut commands: Commands,
    sheet: Res<PlayerSpritesheet>,
    library: Res<AnimationLibrary>,
) {
    let idle_id = library
        .animation_with_name("player:idle")
        .expect("missing animation: player:idle");

    let clips = AnimClips {
        idle: idle_id,
        walk: library.animation_with_name("player:walk"),
        run: library.animation_with_name("player:run"),
        jump: library.animation_with_name("player:jump"),
        fall: library.animation_with_name("player:fall"),
        attack: library.animation_with_name("player:attack"),
    };

    let mut sprite = Sprite::from_atlas_image(
        sheet.image.clone(),
        TextureAtlas { layout: sheet.layout.clone(), ..Default::default() },
    );
    sprite.anchor = Anchor::Custom(Vec2::new(0.0, -0.25));

    let input = InputMap::default()
        .with_axis(Action::Move, VirtualAxis::horizontal_arrow_keys())
        .with_axis(Action::Move, GamepadControlAxis::new(GamepadAxis::LeftStickX))
        .with(Action::Jump, KeyCode::Space)
        .with(Action::Jump, GamepadButton::South)
        .with(Action::Attack, KeyCode::KeyJ)
        .with(Action::Attack, GamepadButton::West)
        .with(Action::Sprint, KeyCode::ShiftLeft)
        .with(Action::Sprint, GamepadButton::LeftTrigger);

    commands.spawn((
        Actor,
        input,
        clips,
        Grounded::Idle,
        StateMachine::default()
            .trans::<Grounded, _>(
                just_pressed(Action::Jump),
                Falling { velocity: JUMP_VELOCITY, vel_x: 0.0 },
            )
            .trans::<Falling, _>(grounded, Grounded::Idle)
            .trans_builder(value_unbounded(Action::Move), |t: Trans<Grounded, f32>| {
                let v = t.out;
                if v > 0.5 { Grounded::Right }
                else if v < -0.5 { Grounded::Left }
                else { Grounded::Idle }
            })
            .trans::<Grounded, _>(just_pressed(Action::Attack), Attacking::default())
            .trans::<Falling,  _>(just_pressed(Action::Attack), Attacking::default())
            .trans::<Attacking, _>(just_released(Action::Attack), Grounded::Idle)
        ,
        sprite,
        SpritesheetAnimation::from_id(idle_id),
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

pub fn anim_on_grounded_change(
    mut q: Query<(&Grounded, &AnimClips, &mut SpritesheetAnimation, &mut Transform), Changed<Grounded>>,
) {
    for (g, clips, mut anim, mut tf) in &mut q {
        let target = match g {
            Grounded::Idle => Some(clips.idle),
            Grounded::Left | Grounded::Right => clips.walk.or(Some(clips.idle)),
        };

        if let Some(id) = target {
            if anim.animation_id != id {
                anim.switch(id);
            }
            anim.playing = true;
        }
        match g {
            Grounded::Right => tf.scale.x = tf.scale.x.abs(),
            Grounded::Left  => tf.scale.x = -tf.scale.x.abs(),
            Grounded::Idle  => {}
        }
    }
}

pub fn anim_refresh_walk_vs_run(
    mut q: Query<(&Grounded, &ActionState<Action>, &AnimClips, &mut SpritesheetAnimation)>,
) {
    for (g, actions, clips, mut anim) in &mut q {
        if !matches!(g, Grounded::Left | Grounded::Right) { continue; }

        let sprinting = actions.pressed(&Action::Sprint);
        let desired = if sprinting {
            clips.run.or(clips.walk)
        } else {
            clips.walk
        }.or(Some(clips.idle));

        if let Some(id) = desired {
            if anim.animation_id != id {
                anim.switch(id);
            }
            anim.playing = true;
        }
    }
}

pub fn anim_on_enter_falling(
    mut q: Query<(&AnimClips, &mut SpritesheetAnimation), Added<Falling>>,
) {
    for (clips, mut anim) in &mut q {
        let id = clips.jump.or(clips.fall).unwrap_or(clips.idle);
        if anim.animation_id != id {
            anim.switch(id);
        }
        anim.playing = true;
    }
}

pub fn anim_on_enter_attacking(
    mut q: Query<(&AnimClips, &mut SpritesheetAnimation), Added<Attacking>>,
) {
    for (clips, mut anim) in &mut q {
        let id = clips.attack.unwrap_or(clips.idle);
        if anim.animation_id != id {
            anim.switch(id);
        }
        anim.playing = true;
    }
}

pub fn init_fall_inertia(
    mut q: Query<(&mut Falling, Option<&Grounded>, &ActionState<Action>), Added<Falling>>,
) {
    for (mut falling, grounded, actions) in &mut q {
        let axis = actions.value(&Action::Move);
        let dir_from_grounded = grounded.map(|g| *g as i32 as f32).unwrap_or(0.0);
        let dir = if axis.abs() > 0.05 { axis } else { dir_from_grounded.signum() };

        let sprinting = actions.pressed(&Action::Sprint);
        let speed = PLAYER_SPEED * if sprinting { SPRINT_MULTIPLIER } else { 1.0 };

        falling.vel_x = speed * dir;
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                anim_on_grounded_change,
                anim_refresh_walk_vs_run,
                anim_on_enter_falling,
                anim_on_enter_attacking,
                init_fall_inertia,
            ),
        );
    }
}