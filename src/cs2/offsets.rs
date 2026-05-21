use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

const OFFSETS_URL: &str =
    "https://raw.githubusercontent.com/a2x/cs2-dumper/main/output/offsets.json";
const CLIENT_SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/a2x/cs2-dumper/main/output/client_dll.json";

#[derive(Debug, Clone, Copy)]
pub struct Offsets {
    pub dw_entity_list: i64,
    pub dw_game_rules: i64,
    pub dw_local_player_controller: i64,
    pub dw_local_player_pawn: i64,

    pub m_h_pawn: i64,
    pub m_p_action_tracking_services: i64,
    pub m_i_num_round_kills: i64,
    pub m_p_weapon_services: i64,
    pub m_h_active_weapon: i64,
    pub m_attribute_manager: i64,
    pub m_f_accuracy_penalty: i64,
    pub m_item: i64,
    pub m_i_item_definition_index: i64,
    pub m_i_health: i64,
    pub m_i_team_num: i64,
    pub m_life_state: i64,

    pub m_b_freeze_period: i64,
    pub m_b_warmup_period: i64,
    pub m_i_round_win_status: i64,
    pub m_b_switching_teams_at_round_reset: i64,
    pub m_b_team_intro_period: i64,
}

#[derive(Deserialize)]
struct ClassSchema {
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    fields: HashMap<String, i64>,
}

#[derive(Deserialize)]
struct ModuleSchema {
    #[serde(default)]
    classes: HashMap<String, ClassSchema>,
}

fn http_get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build();
    let resp = agent
        .get(url)
        .call()
        .with_context(|| format!("HTTP GET {url}"))?;
    resp.into_json::<T>()
        .with_context(|| format!("decode JSON from {url}"))
}

fn merged_class_fields<'a>(
    classes: &'a HashMap<String, ClassSchema>,
    class_name: &str,
) -> HashMap<&'a str, i64> {
    let mut chain: Vec<&str> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut cur: Option<&str> = Some(class_name);
    while let Some(name) = cur {
        if seen.contains(name) {
            break;
        }
        let Some((key, cls)) = classes.get_key_value(name) else {
            break;
        };
        seen.insert(key.as_str());
        chain.push(key.as_str());
        cur = cls.parent.as_deref();
    }
    let mut out: HashMap<&str, i64> = HashMap::new();
    for name in chain.iter().rev() {
        if let Some(cls) = classes.get(*name) {
            for (k, v) in &cls.fields {
                out.insert(k.as_str(), *v);
            }
        }
    }
    out
}

pub fn fetch_offsets() -> Result<Offsets> {
    let raw_off: HashMap<String, HashMap<String, serde_json::Value>> =
        http_get_json(OFFSETS_URL).context("fetching offsets.json")?;
    let schema: HashMap<String, ModuleSchema> =
        http_get_json(CLIENT_SCHEMA_URL).context("fetching client_dll.json")?;

    let c_client = raw_off
        .get("client.dll")
        .ok_or_else(|| anyhow!("offsets.json: missing `client.dll`"))?;
    let classes = &schema
        .get("client.dll")
        .ok_or_else(|| anyhow!("client_dll.json: missing `client.dll`"))?
        .classes;

    let get_off = |name: &str| -> Result<i64> {
        c_client
            .get(name)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("offsets.json: missing client.dll.{name}"))
    };

    let get_field = |cls: &str, name: &str| -> Result<i64> {
        merged_class_fields(classes, cls)
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("client_dll.json: {cls}.{name} not in schema"))
    };

    Ok(Offsets {
        dw_entity_list: get_off("dwEntityList")?,
        dw_game_rules: get_off("dwGameRules")?,
        dw_local_player_controller: get_off("dwLocalPlayerController")?,
        dw_local_player_pawn: get_off("dwLocalPlayerPawn")?,

        m_h_pawn: get_field("CBasePlayerController", "m_hPawn")?,
        m_p_action_tracking_services: get_field(
            "CCSPlayerController",
            "m_pActionTrackingServices",
        )?,
        m_i_num_round_kills: get_field(
            "CCSPlayerController_ActionTrackingServices",
            "m_iNumRoundKills",
        )?,
        m_p_weapon_services: get_field("C_BasePlayerPawn", "m_pWeaponServices")?,
        m_h_active_weapon: get_field("CPlayer_WeaponServices", "m_hActiveWeapon")?,
        m_attribute_manager: get_field("C_EconEntity", "m_AttributeManager")?,
        m_f_accuracy_penalty: get_field("C_CSWeaponBase", "m_fAccuracyPenalty")?,
        m_item: get_field("C_AttributeContainer", "m_Item")?,
        m_i_item_definition_index: get_field("C_EconItemView", "m_iItemDefinitionIndex")?,
        m_i_health: get_field("C_BaseEntity", "m_iHealth")?,
        m_i_team_num: get_field("C_BaseEntity", "m_iTeamNum")?,
        m_life_state: get_field("C_BaseEntity", "m_lifeState")?,

        m_b_freeze_period: get_field("C_CSGameRules", "m_bFreezePeriod")?,
        m_b_warmup_period: get_field("C_CSGameRules", "m_bWarmupPeriod")?,
        m_i_round_win_status: get_field("C_CSGameRules", "m_iRoundWinStatus")?,
        m_b_switching_teams_at_round_reset: get_field(
            "C_CSGameRules",
            "m_bSwitchingTeamsAtRoundReset",
        )?,
        m_b_team_intro_period: get_field("C_CSGameRules", "m_bTeamIntroPeriod")?,
    })
}
