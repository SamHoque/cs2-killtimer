use crate::cs2::offsets::Offsets;
use crate::utils::memory::Handle;

/// Stride between consecutive entries inside a Source 2 entity-list chunk.
const ENTITY_STRIDE: i64 = 0x70;

/// Return a null handle bound to the same `Process` as `anchor`. The `Handle`
/// type doesn't expose a constructor, so we land on addr 0 via wrapping
/// arithmetic. Safe for any user-mode address (< 2^63).
fn null_like(anchor: Handle) -> Handle {
    anchor.sub(anchor.addr() as i64)
}

pub fn get_entity(list: Handle, idx: i32) -> Handle {
    if idx <= 0 || idx >= 8192 {
        return null_like(list);
    }
    let chunk_off: i64 = 8 * (((idx & 0x7FFF) >> 9) as i64) + 0x10;
    let entry_off: i64 = ENTITY_STRIDE * ((idx & 0x1FF) as i64);
    // chunk = *(u64*)(list + chunk_off); ent = *(u64*)(chunk + entry_off)
    // a failed read along the chain yields addr 0, naturally null-propagating.
    list.add(chunk_off).deref().add(entry_off).deref()
}

pub fn controller_to_pawn<'p>(list: Handle<'p>, ctrl: Handle<'p>, off: &Offsets) -> Handle<'p> {
    if ctrl.is_null() {
        return null_like(list);
    }
    let h: u32 = ctrl.add(off.m_h_pawn).read_or();
    get_entity(list, (h & 0x7FFF) as i32)
}

pub fn round_kills(ctrl: Handle, off: &Offsets) -> i32 {
    if ctrl.is_null() {
        return 0;
    }
    ctrl.add(off.m_p_action_tracking_services)
        .deref()
        .add(off.m_i_num_round_kills)
        .read_or::<i32>()
}

pub fn active_weapon_id<'p>(list: Handle<'p>, pawn: Handle<'p>, off: &Offsets) -> u16 {
    if pawn.is_null() {
        return 0;
    }
    let ws = pawn.add(off.m_p_weapon_services).deref();
    if ws.is_null() {
        return 0;
    }
    let h: u32 = ws.add(off.m_h_active_weapon).read_or();
    let weapon = get_entity(list, (h & 0x7FFF) as i32);
    if weapon.is_null() {
        return 0;
    }
    weapon
        .add(off.m_attribute_manager)
        .add(off.m_item)
        .add(off.m_i_item_definition_index)
        .read_or::<u16>()
}

pub fn is_alive(pawn: Handle, off: &Offsets) -> bool {
    if pawn.is_null() {
        return false;
    }
    let hp: i32 = pawn.add(off.m_i_health).read_or();
    let life: u8 = pawn.add(off.m_life_state).read_or();
    hp > 0 && life == 0
}

/// Pawn's team number (0 = unassigned, 1 = spec, 2 = T, 3 = CT).
pub fn team(pawn: Handle, off: &Offsets) -> u8 {
    if pawn.is_null() {
        return 0;
    }
    pawn.add(off.m_i_team_num).read_or::<u8>()
}

/// True only when the round is in active play (no warmup/freeze/team-intro/
/// post-win).
pub fn is_round_live(client: Handle, off: &Offsets) -> bool {
    let rules = client.add(off.dw_game_rules).deref();
    if rules.is_null() {
        return false;
    }
    let warmup: u8 = rules.add(off.m_b_warmup_period).read_or();
    let freeze: u8 = rules.add(off.m_b_freeze_period).read_or();
    if warmup != 0 || freeze != 0 {
        return false;
    }
    let switching: u8 = rules.add(off.m_b_switching_teams_at_round_reset).read_or();
    if switching != 0 {
        return false;
    }
    let intro: u8 = rules.add(off.m_b_team_intro_period).read_or();
    if intro != 0 {
        return false;
    }
    let win_status: i32 = rules.add(off.m_i_round_win_status).read_or();
    win_status == 0
}

/// Count enemies still alive (team != local_team, hp > 0, lifeState == 0).
/// Iterates controllers 1..64.
pub fn enemies_alive<'p>(
    list: Handle<'p>,
    local_pawn: Handle<'p>,
    local_team: u8,
    off: &Offsets,
) -> u32 {
    if local_team <= 1 || local_pawn.is_null() {
        return 0;
    }
    let mut n: u32 = 0;
    for i in 1..64 {
        let ctrl = get_entity(list, i);
        if ctrl.is_null() {
            continue;
        }
        let pawn = controller_to_pawn(list, ctrl, off);
        if pawn.is_null() || pawn.addr() == local_pawn.addr() {
            continue;
        }
        let t = team(pawn, off);
        if t <= 1 || t == local_team {
            continue;
        }
        if is_alive(pawn, off) {
            n += 1;
        }
    }
    n
}

/// Workaround: in current cs2-dumper builds, `dwLocalPlayerController` doesn't
/// store a usable pointer (handle-shaped value with garbage upper bits). The
/// local pawn pointer still works, so we find the controller via the pawn.
pub fn find_local_controller<'p>(
    list: Handle<'p>,
    local_pawn: Handle<'p>,
    off: &Offsets,
) -> Handle<'p> {
    let target = local_pawn.addr();
    if target == 0 {
        return null_like(list);
    }
    for i in 1..64 {
        let ctrl = get_entity(list, i);
        if ctrl.is_null() {
            continue;
        }
        let p = controller_to_pawn(list, ctrl, off);
        if !p.is_null() && p.addr() == target {
            return ctrl;
        }
    }
    null_like(list)
}
