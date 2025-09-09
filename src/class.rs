use crate::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Reflect, Resource)]
pub struct ClassFile {
    pub id: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub attribute_start: Attributes,
    pub base_stats: BaseStats,
}

#[derive(Debug, Clone, Deserialize, Reflect)]
pub struct Attributes {
    pub might: u32,
    pub agility: u32,
    pub focus: u32,
    pub grit: u32,
}

#[derive(Debug, Clone, Deserialize, Reflect)]
pub struct BaseStats {
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

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ClassAttachTarget;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct PlayerClass(pub ClassFile);

#[derive(Resource, Clone)]
pub struct ClassPluginConfig {
    pub path: String,
    pub spawn_debug_holder_if_missing: bool,
}

pub struct ClassPlugin {
    config: ClassPluginConfig,
}

impl ClassPlugin {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            config: ClassPluginConfig {
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

impl Plugin for ClassPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .register_type::<ClassAttachTarget>()
            .register_type::<PlayerClass>()
            .register_type::<ClassFile>()
            .register_type::<Attributes>()
            .register_type::<BaseStats>()
            .add_systems(PreStartup, (load_class_from_json, maybe_spawn_debug_holder))
            .add_systems(Update, attach_class_to_targets);
    }
}

fn load_class_from_json(mut commands: Commands, cfg: Res<ClassPluginConfig>) {
    let path = &cfg.path;
    let json = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("Failed to read class JSON at {path}: {e}");
    });
    let class_file: ClassFile = serde_json::from_str(&json).unwrap_or_else(|e| {
        panic!("Invalid class JSON format for {path}: {e}");
    });

    commands.insert_resource(class_file);
}

fn maybe_spawn_debug_holder(
    mut commands: Commands,
    cfg: Res<ClassPluginConfig>,
    q_targets: Query<Entity, With<ClassAttachTarget>>,
) {
    if !cfg.spawn_debug_holder_if_missing {
        return;
    }
    if q_targets.is_empty() {
        commands.spawn((
            Name::new("ClassHolder (Debug)"),
            ClassAttachTarget,
            Transform::default(),
            GlobalTransform::default(),
        ));
        info!(
            "ClassPlugin: Spawned debug holder entity (ClassAttachTarget). Add `ClassAttachTarget` to your real player to attach there instead."
        );
    }
}

fn attach_class_to_targets(
    class_file: Option<Res<ClassFile>>,
    mut commands: Commands,
    q_targets: Query<(Entity, Option<&PlayerClass>), With<ClassAttachTarget>>,
) {
    let Some(class_file) = class_file else { return };
    for (e, maybe_existing) in &q_targets {
        if maybe_existing.is_none() {
            commands.entity(e).insert(PlayerClass(class_file.clone()));
        }
    }
}
