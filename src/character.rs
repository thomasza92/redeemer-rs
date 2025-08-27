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

#[derive(Component, Deref, DerefMut)]
pub struct AttackTimer(pub Timer);

#[derive(Component, Clone, Copy)]
pub struct AnimClips {
    pub idle: AnimationId,
    pub walk: Option<AnimationId>,
    pub run:  Option<AnimationId>,
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
        walk:  library.animation_with_name("player:walk"),
        run:   library.animation_with_name("player:run"),
        jump:  library.animation_with_name("player:jump"),
        fall:  library.animation_with_name("player:fall"),
        attack:library.animation_with_name("player:attack"),
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
    mut q: Query<
        (&Grounded, &AnimClips, &mut SpritesheetAnimation, &mut Transform),
        (Changed<Grounded>, Without<Attacking>),
    >,
) {
    for (g, clips, mut anim, mut tf) in &mut q {
        match g {
            Grounded::Idle => {
                let id = clips.idle;
                if anim.animation_id != id {
                    anim.switch(id);
                }
                anim.playing = true;
            }
            Grounded::Left => {
                tf.scale.x = -tf.scale.x.abs();
                anim.playing = true;
            }
            Grounded::Right => {
                tf.scale.x = tf.scale.x.abs();
                anim.playing = true;
            }
        }
    }
}

pub fn anim_refresh_walk_vs_run(
    mut q: Query<
        (&Grounded, &ActionState<Action>, &AnimClips, &mut SpritesheetAnimation),
        Without<Attacking>,
    >,
) {
    for (g, actions, clips, mut anim) in &mut q {
        if !matches!(g, Grounded::Left | Grounded::Right) { continue; }

        let sprinting = actions.pressed(&Action::Sprint);
        let desired = if sprinting {
            clips.run.or(clips.walk).unwrap_or(clips.idle)
        } else {
            clips.walk.unwrap_or(clips.idle)
        };

        if anim.animation_id != desired {
            anim.switch(desired);
        }
        anim.playing = true;
    }
}

pub fn anim_on_enter_falling(
    mut q: Query<(&AnimClips, &Falling, &mut SpritesheetAnimation, &mut Transform), (Added<Falling>, Without<Attacking>)>,
) {
    for (clips, falling, mut anim, mut tf) in &mut q {
        let id = clips.jump.or(clips.fall).unwrap_or(clips.idle);
        if anim.animation_id != id {
            anim.switch(id);
        }
        anim.playing = true;

        if falling.vel_x > 0.0 {
            tf.scale.x = tf.scale.x.abs();
        } else if falling.vel_x < 0.0 {
            tf.scale.x = -tf.scale.x.abs();
        }
    }
}

const ATTACK_SECONDS: f32 = 0.50;

pub fn start_attack(
    mut commands: Commands,
    q: Query<(Entity, &ActionState<Action>, &AnimClips, &SpritesheetAnimation), Without<Attacking>>,
) {
    for (e, actions, clips, anim) in &q {
        if !actions.just_pressed(&Action::Attack) { continue; }
        let id = clips.attack.unwrap_or(clips.idle);
        if anim.animation_id != id {
            commands.entity(e).insert(QueuedAnimSwitch(id));
        }
        commands.entity(e).insert((
            Attacking,
            AttackTimer(Timer::from_seconds(ATTACK_SECONDS, TimerMode::Once)),
        ));
    }
}

#[derive(Component)]
struct QueuedAnimSwitch(AnimationId);

fn apply_queued_switch(mut commands: Commands, mut q: Query<(Entity, &QueuedAnimSwitch, &mut SpritesheetAnimation)>) {
    for (e, queued, mut anim) in &mut q {
        if anim.animation_id != queued.0 {
            anim.switch(queued.0);
        }
        anim.playing = true;
        commands.entity(e).remove::<QueuedAnimSwitch>();
    }
}

pub fn update_attack_timer(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut AttackTimer)>,
) {
    for (e, mut timer) in &mut q {
        timer.tick(time.delta());
        if timer.finished() {
            commands.entity(e).remove::<(Attacking, AttackTimer)>();
        }
    }
}

pub fn interrupt_attack_on_jump_or_fall(
    mut commands: Commands,
    q: Query<Entity, (With<Attacking>, Added<Falling>)>,
) {
    for e in &q {
        commands.entity(e).remove::<(Attacking, AttackTimer)>();
    }
}

pub fn interrupt_attack_on_move(
    mut commands: Commands,
    q: Query<(Entity, &Grounded), (With<Attacking>, Changed<Grounded>)>,
) {
    for (e, g) in &q {
        if matches!(g, Grounded::Left | Grounded::Right) {
            commands.entity(e).remove::<(Attacking, AttackTimer)>();
        }
    }
}

pub fn init_fall_inertia(
    mut q: Query<(&mut Falling, &ActionState<Action>, &mut Transform), Added<Falling>>,
) {
    const MOVE_THRESHOLD: f32 = 0.5;

    for (mut falling, actions, mut tf) in &mut q {
        let axis = actions.value(&Action::Move);
        let dir = if axis > MOVE_THRESHOLD {
            1.0
        } else if axis < -MOVE_THRESHOLD {
            -1.0
        } else {
            0.0
        };

        let sprinting = actions.pressed(&Action::Sprint);
        let speed = PLAYER_SPEED * if sprinting { SPRINT_MULTIPLIER } else { 1.0 };
        falling.vel_x = speed * dir;
        
        if dir > 0.0 {
            tf.scale.x = tf.scale.x.abs();
        } else if dir < 0.0 {
            tf.scale.x = -tf.scale.x.abs();
        }
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
                init_fall_inertia,
                start_attack,
                apply_queued_switch,
                update_attack_timer,
                interrupt_attack_on_jump_or_fall,
                interrupt_attack_on_move,
            ),
        );
    }
}