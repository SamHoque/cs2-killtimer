use std::borrow::Cow;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Kind {
    Firearm,
    NonBullet,
    Unknown,
}

pub struct Weapon {
    pub id: u16,
    pub name: &'static str,
    pub kind: Kind,
}

use Kind::*;

const fn w(id: u16, name: &'static str, kind: Kind) -> Weapon {
    Weapon { id, name, kind }
}

// Sorted by id. Single source of truth for known weapons.
const TABLE: &[Weapon] = &[
    w(1, "deagle", Firearm),
    w(2, "dual_berettas", Firearm),
    w(3, "five_seven", Firearm),
    w(4, "glock", Firearm),
    w(7, "ak47", Firearm),
    w(8, "aug", Firearm),
    w(9, "awp", Firearm),
    w(10, "famas", Firearm),
    w(11, "g3sg1", Firearm),
    w(13, "galil_ar", Firearm),
    w(14, "m249", Firearm),
    w(16, "m4a4", Firearm),
    w(17, "mac10", Firearm),
    w(19, "p90", Firearm),
    w(23, "mp5sd", Firearm),
    w(24, "ump45", Firearm),
    w(25, "xm1014", Firearm),
    w(26, "bizon", Firearm),
    w(27, "mag7", Firearm),
    w(28, "negev", Firearm),
    w(29, "sawedoff", Firearm),
    w(30, "tec9", Firearm),
    w(31, "taser", NonBullet),
    w(32, "p2000", Firearm),
    w(33, "mp7", Firearm),
    w(34, "mp9", Firearm),
    w(35, "nova", Firearm),
    w(36, "p250", Firearm),
    w(37, "shield", NonBullet),
    w(38, "scar20", Firearm),
    w(39, "sg553", Firearm),
    w(40, "ssg08", Firearm),
    w(41, "knifegg", NonBullet),
    w(42, "knife", NonBullet),
    w(43, "flashbang", NonBullet),
    w(44, "hegrenade", NonBullet),
    w(45, "smokegrenade", NonBullet),
    w(46, "molotov", NonBullet),
    w(47, "decoy", NonBullet),
    w(48, "incgrenade", NonBullet),
    w(49, "c4", NonBullet),
    w(57, "healthshot", NonBullet),
    w(59, "knife_t", NonBullet),
    w(60, "m4a1s", Firearm),
    w(61, "usp_s", Firearm),
    w(63, "cz75_auto", Firearm),
    w(64, "r8_revolver", Firearm),
    w(68, "tagrenade", NonBullet),
    w(69, "fists", NonBullet),
    w(70, "breachcharge", NonBullet),
    w(72, "tablet", NonBullet),
    w(75, "axe", NonBullet),
    w(76, "hammer", NonBullet),
    w(78, "spanner", NonBullet),
    w(80, "knife_ghost", NonBullet),
    w(81, "firebomb", NonBullet),
    w(82, "diversion", NonBullet),
    w(83, "frag_grenade", NonBullet),
    w(84, "snowball", NonBullet),
    w(85, "bumpmine", NonBullet),
    w(500, "bayonet", NonBullet),
    w(503, "knife_css", NonBullet),
    w(505, "knife_flip", NonBullet),
    w(506, "knife_gut", NonBullet),
    w(507, "knife_karambit", NonBullet),
    w(508, "knife_m9_bayonet", NonBullet),
    w(509, "knife_tactical", NonBullet),
    w(512, "knife_falchion", NonBullet),
    w(514, "knife_survival_bowie", NonBullet),
    w(515, "knife_butterfly", NonBullet),
    w(516, "knife_push", NonBullet),
    w(517, "knife_cord", NonBullet),
    w(518, "knife_canis", NonBullet),
    w(519, "knife_ursus", NonBullet),
    w(520, "knife_gypsy_jackknife", NonBullet),
    w(521, "knife_outdoor", NonBullet),
    w(522, "knife_stiletto", NonBullet),
    w(523, "knife_widowmaker", NonBullet),
    w(525, "knife_skeleton", NonBullet),
    w(526, "knife_kukri", NonBullet),
];

// Substring fallbacks for ids that don't appear in TABLE (forward-compat).
const FIREARM_HINTS: &[&str] = &[
    "ak47", "aug", "awp", "bizon", "cz75", "deagle", "elite", "famas", "fiveseven", "g3sg1",
    "galil", "glock", "hkp2000", "m249", "m4a1", "mac10", "mag7", "mp5", "mp7", "mp9", "negev",
    "nova", "p2000", "p250", "p90", "revolver", "r8", "sawedoff", "scar20", "sg553", "sg556",
    "ssg08", "tec9", "ump45", "usp", "xm1014",
];

const NON_BULLET_HINTS: &[&str] = &[
    "axe", "bayonet", "bomb", "breachcharge", "bumpmine", "c4", "decoy", "diversion", "explosion",
    "firebomb", "fists", "frag_grenade", "grenade", "hammer", "hegrenade", "incgrenade",
    "incendiary", "inferno", "knife", "knifegg", "melee", "molly", "molotov", "nade", "planted_c4",
    "shield", "smokegrenade", "snowball", "spanner", "stomp_damage", "tagrenade", "tablet",
    "taser", "tripwire", "zeus",
];

pub fn lookup(id: u16) -> Option<&'static Weapon> {
    TABLE
        .binary_search_by_key(&id, |w| w.id)
        .ok()
        .map(|i| &TABLE[i])
}

pub fn display_name(id: u16) -> Cow<'static, str> {
    lookup(id).map_or_else(
        || Cow::Owned(format!("item_{id}")),
        |w| Cow::Borrowed(w.name),
    )
}

pub fn classify(id: u16) -> Kind {
    if let Some(w) = lookup(id) {
        return w.kind;
    }
    // 500-599 is the knife range; default to NonBullet for unknown ids in it.
    if (500..=599).contains(&id) {
        return NonBullet;
    }
    let lower = display_name(id).to_lowercase();
    if FIREARM_HINTS.iter().any(|h| lower.contains(h)) {
        return Firearm;
    }
    if NON_BULLET_HINTS.iter().any(|h| lower.contains(h)) {
        return NonBullet;
    }
    Unknown
}

pub fn is_kill_ignored(id: u16) -> bool {
    !matches!(classify(id), Firearm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sorted_by_id() {
        assert!(TABLE.windows(2).all(|p| p[0].id < p[1].id));
    }

    #[test]
    fn classification() {
        assert_eq!(classify(7), Firearm); // ak47
        assert_eq!(classify(31), NonBullet); // taser
        assert_eq!(classify(44), NonBullet); // hegrenade
        assert_eq!(classify(525), NonBullet); // knife
        assert_eq!(classify(501), NonBullet); // unknown knife range
        assert_eq!(classify(9999), Unknown);
        assert!(is_kill_ignored(31));
        assert!(!is_kill_ignored(7));
    }

    #[test]
    fn display() {
        assert_eq!(display_name(7), "ak47");
        assert_eq!(display_name(9999), "item_9999");
    }
}
