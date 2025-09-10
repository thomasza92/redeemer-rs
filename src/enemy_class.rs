// enemy_class.rs
use crate::prelude::*;
use serde::Deserialize;

/// Top-level enemy class file loaded from JSON.
#[derive(Debug, Clone, Deserialize, Reflect, Resource)]
pub struct EnemyClassFile {
    pub id: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub attribute_start: EnemyAttributes,
    pub base_stats: EnemyBaseStats,
}

#[derive(Debug, Clone, Deserialize, Reflect)]
pub struct EnemyAttributes {
    pub might: u32,
    pub agility: u32,
    pub focus: u32,
    pub grit: u32,
}

#[derive(Debug, Clone, Deserialize, Reflect)]
pub struct EnemyBaseStats {
    pub max_health: u32,
    pub defense: f32,
    pub knockback_resist: f32,
    pub melee_power: f32,
    pub spell_power: f32,
    pub knockback: f32,
    pub move_speed: f32,
    pub crit_chance: f32,
    pub crit_multiplier: f32,
    pub attack_cooldown_reduction: f32,
    pub projectile_speed: f32,
    pub stamina_max: f32,
    pub stamina_regen_per_s: f32,
}

/// Tag any enemy entity you want this EnemyClass attached to.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct EnemyClassAttachTarget;

/// Component attached to enemies with their class data.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct EnemyClass(pub EnemyClassFile);

#[derive(Resource, Clone)]
pub struct EnemyClassPluginConfig {
    pub path: String,
    pub spawn_debug_holder_if_missing: bool,
}

pub struct EnemyClassPlugin {
    config: EnemyClassPluginConfig,
}

impl EnemyClassPlugin {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            config: EnemyClassPluginConfig {
                path: path.into(),
                spawn_debug_holder_if_missing: true,
            },
        }
    }

    pub fn spawn_debug_holder(mut self, enabled: bool) -> Self {
        self.config.spawn_debug_holder_if_missing = enabled;
        self
    }
}

impl Plugin for EnemyClassPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .register_type::<EnemyClassAttachTarget>()
            .register_type::<EnemyClass>()
            .register_type::<EnemyClassFile>()
            .register_type::<EnemyAttributes>()
            .register_type::<EnemyBaseStats>()
            .add_systems(
                PreStartup,
                (load_enemy_class_from_json, maybe_spawn_debug_holder),
            )
            .add_systems(Update, attach_enemy_class_to_targets);
    }
}

fn load_enemy_class_from_json(mut commands: Commands, cfg: Res<EnemyClassPluginConfig>) {
    let path = &cfg.path;
    let json = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("Failed to read enemy class JSON at {path}: {e}");
    });
    let class_file: EnemyClassFile = serde_json::from_str(&json).unwrap_or_else(|e| {
        panic!("Invalid enemy class JSON format for {path}: {e}");
    });

    commands.insert_resource(class_file);
}

fn maybe_spawn_debug_holder(
    mut commands: Commands,
    cfg: Res<EnemyClassPluginConfig>,
    q_targets: Query<Entity, With<EnemyClassAttachTarget>>,
) {
    if !cfg.spawn_debug_holder_if_missing {
        return;
    }
    if q_targets.is_empty() {
        commands.spawn((
            Name::new("EnemyClassHolder (Debug)"),
            EnemyClassAttachTarget,
            Transform::default(),
            GlobalTransform::default(),
        ));
        info!(
            "EnemyClassPlugin: Spawned debug holder entity (EnemyClassAttachTarget). \
             Add `EnemyClassAttachTarget` to your real enemy to attach there instead."
        );
    }
}

fn attach_enemy_class_to_targets(
    class_file: Option<Res<EnemyClassFile>>,
    mut commands: Commands,
    q_targets: Query<(Entity, Option<&EnemyClass>), With<EnemyClassAttachTarget>>,
) {
    let Some(class_file) = class_file else { return };
    for (e, maybe_existing) in &q_targets {
        if maybe_existing.is_none() {
            commands.entity(e).insert(EnemyClass(class_file.clone()));
        }
    }
}
