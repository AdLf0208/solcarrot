// Spice of Life: Carrot Edition for Horizon — the pure tracking core.
//
// Ported from Spice of Life: Carrot Edition 1.21.1-1.16.6
// (Cazsius/Spice-of-Life-Carrot-Edition, LGPL-2.1) for Minecraft 1.21.1
// NeoForge. Upstream source read for behaviour only; this is a clean-room Rust
// rewrite.
//
// The mod records every food a player has eaten (by item id), counts the
// unique foods that pass the nutrition/filter rules, and grants a max-health
// modifier at each configured milestone. Nothing here touches the game: the
// glue reads the item registry, stores this history under the world folder and
// applies the modifier to the acting player.

use std::collections::{BTreeMap, BTreeSet};

/// Upstream `SOLCarrotConfig.Server` defaults (`SOLCarrotConfig.java:135-181`).
pub const BASE_HEARTS_DEFAULT: i32 = 10;
pub const HEARTS_PER_MILESTONE_DEFAULT: i32 = 2;
pub const MINIMUM_FOOD_VALUE_DEFAULT: i32 = 1;
pub const MILESTONES_DEFAULT: [i32; 5] = [5, 10, 15, 20, 25];

/// Upstream's permanent modifier id (`MaxHealthHandler.java:21`).
pub const HEALTH_MODIFIER_ID: &str = "solcarrot:health_gained";

/// Where the glue keeps the history under the world folder and the config
/// beside the game's `options.txt` (the recipe is the mod's own, not upstream's
/// NBT attachment; see the port's notes).
pub const STORE_DIR: &str = "solcarrot";
pub const STORE_FILE: &str = "foods.tsv";
pub const CONFIG_FILE: &str = "solcarrot.conf";
pub const STORE_HEADER: &str = "# solcarrot v1";

/// The server-side settings this slice understands. Defaults are upstream's.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Hearts a player starts with; the max-health base stays vanilla's 20 and
    /// the difference becomes part of this mod's modifier (upstream
    /// `MaxHealthHandler.java:40`).
    pub base_hearts: i32,
    /// Hearts granted per reached milestone (2 health points each).
    pub hearts_per_milestone: i32,
    /// Ascending numbers of unique foods that unlock each milestone.
    pub milestones: Vec<i32>,
    /// Food ids that never count (wildcards allowed).
    pub blacklist: Vec<String>,
    /// When non-empty, only these food ids count and the blacklist is ignored.
    pub whitelist: Vec<String>,
    /// Minimum nutrition (half drumsticks) a food needs to count.
    pub minimum_food_value: i32,
    /// Clear the history when the player dies.
    pub reset_on_death: bool,
    /// Do not track food eaten outside survival mode.
    pub limit_progression_to_survival: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_hearts: BASE_HEARTS_DEFAULT,
            hearts_per_milestone: HEARTS_PER_MILESTONE_DEFAULT,
            milestones: MILESTONES_DEFAULT.to_vec(),
            blacklist: Vec::new(),
            whitelist: Vec::new(),
            minimum_food_value: MINIMUM_FOOD_VALUE_DEFAULT,
            reset_on_death: false,
            limit_progression_to_survival: false,
        }
    }
}

impl Config {
    /// The highest milestone, or None when the list is empty. Upstream panics
    /// on an empty list (`SOLCarrotConfig.java:270`); a rewrite treats it as
    /// "no bonus available".
    pub fn highest_milestone(&self) -> Option<i32> {
        self.milestones.iter().copied().max()
    }
}

/// Parse `config/solcarrot.conf`: `key = value` lines, `#` comments, commas
/// for lists. Unknown keys and badly typed values keep their defaults.
pub fn parse_config(text: &str) -> Config {
    let mut config = Config::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "base_hearts" => {
                if let Ok(v) = value.parse::<i32>() {
                    config.base_hearts = v.clamp(0, 1000);
                }
            }
            "hearts_per_milestone" => {
                if let Ok(v) = value.parse::<i32>() {
                    config.hearts_per_milestone = v.clamp(0, 1000);
                }
            }
            "milestones" => config.milestones = parse_int_list(value),
            "blacklist" => config.blacklist = parse_string_list(value),
            "whitelist" => config.whitelist = parse_string_list(value),
            "minimum_food_value" => {
                if let Ok(v) = value.parse::<i32>() {
                    config.minimum_food_value = v.clamp(0, 1000);
                }
            }
            "reset_on_death" => {
                if let Some(v) = parse_bool(value) {
                    config.reset_on_death = v;
                }
            }
            "limit_progression_to_survival" => {
                if let Some(v) = parse_bool(value) {
                    config.limit_progression_to_survival = v;
                }
            }
            _ => {}
        }
    }
    config
}

fn parse_int_list(value: &str) -> Vec<i32> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect()
}

fn parse_string_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Write a config file in the format `parse_config` reads (upstream writes its
/// defaults on first run too).
pub fn format_config(config: &Config) -> String {
    let milestones = config
        .milestones
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "# Spice of Life: Carrot Edition (Horizon) — server settings.\n\
         # `*` matches any run of characters in the filter lists.\n\
         base_hearts = {}\n\
         hearts_per_milestone = {}\n\
         milestones = {milestones}\n\
         blacklist = {}\n\
         whitelist = {}\n\
         minimum_food_value = {}\n\
         reset_on_death = {}\n\
         limit_progression_to_survival = {}\n",
        config.base_hearts,
        config.hearts_per_milestone,
        config.blacklist.join(","),
        config.whitelist.join(","),
        config.minimum_food_value,
        config.reset_on_death,
        config.limit_progression_to_survival,
    )
}

/// Upstream's glob: `*` matches any run of characters, everything else is
/// literal (`SOLCarrotConfig.java:302-320`). The whole id must match.
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let Some(first) = parts.first() else {
        return pattern == value;
    };
    if !value.starts_with(first) {
        return false;
    }
    let mut rest = &value[first.len()..];
    for part in &parts[1..] {
        if part.is_empty() {
            continue;
        }
        let Some(index) = rest.find(part) else {
            return false;
        };
        rest = &rest[index + part.len()..];
    }
    // A trailing `*` (or no part at all) admits any suffix; otherwise the last
    // literal must end the id.
    parts.last().is_some_and(|last| last.is_empty()) || rest.is_empty()
}

fn matches_any(patterns: &[String], id: &str) -> bool {
    patterns.iter().any(|pattern| glob_matches(pattern, id))
}

/// Whether a food may contribute to milestones (`SOLCarrotConfig.shouldCount`):
/// nutrition at the configured minimum and the filter lists agree.
pub fn should_count(item_id: &str, nutrition: i32, config: &Config) -> bool {
    if nutrition < config.minimum_food_value {
        return false;
    }
    if config.whitelist.is_empty() {
        !matches_any(&config.blacklist, item_id)
    } else {
        matches_any(&config.whitelist, item_id)
    }
}

/// Count the stored foods that still pass `should_count`. Missing items (a
/// removed mod) do not count; upstream drops them from its list on load.
pub fn counted_foods<'a>(
    ids: impl IntoIterator<Item = &'a str>,
    nutrition_of: impl Fn(&str) -> Option<i32>,
    config: &Config,
) -> usize {
    ids.into_iter()
        .filter(|id| nutrition_of(id).is_some_and(|n| should_count(id, n, config)))
        .count()
}

/// How many milestones a food count has reached (`ProgressInfo.java:31-34`).
pub fn milestones_achieved(foods_eaten: usize, config: &Config) -> usize {
    let eaten = i32::try_from(foods_eaten).unwrap_or(i32::MAX);
    config
        .milestones
        .iter()
        .filter(|milestone| eaten >= **milestone)
        .count()
}

/// The max-health modifier in health points (`MaxHealthHandler.java:40-46`):
/// the base-hearts difference plus two points per heart per milestone.
pub fn health_modifier_amount(foods_eaten: usize, config: &Config) -> i32 {
    2 * (config.base_hearts - 10)
        + 2 * config.hearts_per_milestone * milestones_achieved(foods_eaten, config) as i32
}

/// The progress upstream shows in its book and `/foodlist size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Unique foods that count.
    pub foods_eaten: usize,
    pub milestones_achieved: usize,
    /// Whether the highest milestone was reached.
    pub has_reached_max: bool,
    /// The next milestone, or None at the cap / with no milestones.
    pub next_milestone: Option<i32>,
    /// Foods still needed for the next milestone.
    pub foods_until_next: Option<i32>,
}

pub fn progress(foods_eaten: usize, config: &Config) -> Progress {
    let eaten = i32::try_from(foods_eaten).unwrap_or(i32::MAX);
    let achieved = milestones_achieved(foods_eaten, config);
    let highest = config.highest_milestone();
    let has_reached_max = highest.is_none_or(|highest| eaten >= highest);
    let next_milestone = if has_reached_max {
        None
    } else {
        config
            .milestones
            .iter()
            .copied()
            .filter(|milestone| *milestone > eaten)
            .min()
    };
    Progress {
        foods_eaten,
        milestones_achieved: achieved,
        has_reached_max,
        next_milestone,
        foods_until_next: next_milestone.map(|next| next - eaten),
    }
}

/// The outcome of adding one eaten food (`FoodList.addFood`,
/// `FoodList.java:68-73`): upstream inserts the item first and only then asks
/// whether it counts, so a filtered food is still remembered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added {
    /// The food was not in the history before.
    pub newly_added: bool,
    /// It passes the nutrition and filter rules.
    pub counts: bool,
}

/// A player's unique-food history. Order-independent and persisted as item ids
/// (upstream stores the item id strings, `FoodInstance.encode`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FoodHistory {
    foods: BTreeSet<String>,
}

#[allow(dead_code)] // the standalone tests and later slices use more than the glue.
impl FoodHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, item: &str, nutrition: i32, config: &Config) -> Added {
        let newly_added = self.foods.insert(item.to_owned());
        Added {
            newly_added,
            counts: should_count(item, nutrition, config),
        }
    }

    pub fn contains(&self, item: &str) -> bool {
        self.foods.contains(item)
    }

    pub fn len(&self) -> usize {
        self.foods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.foods.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.foods.iter().map(String::as_str)
    }

    pub fn clear(&mut self) {
        self.foods.clear();
    }

    pub fn counted(&self, nutrition_of: impl Fn(&str) -> Option<i32>, config: &Config) -> usize {
        counted_foods(self.iter(), nutrition_of, config)
    }

    pub fn from_ids(ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            foods: ids.into_iter().collect(),
        }
    }
}

/// One line per (player, food): `<32 hex digits>\t<item id>`. The format is the
/// port's own (upstream uses a NeoForge player attachment); a tab keeps item
/// ids unambiguous and a header makes a future migration detectable.
pub fn format_store(players: &BTreeMap<u128, FoodHistory>) -> String {
    let mut out = String::from(STORE_HEADER);
    out.push('\n');
    for (uuid, history) in players {
        for item in history.iter() {
            out.push_str(&format_uuid_hex(*uuid));
            out.push('\t');
            out.push_str(item);
            out.push('\n');
        }
    }
    out
}

/// Parse the history file, skipping malformed lines instead of losing the rest.
pub fn parse_store(text: &str) -> BTreeMap<u128, FoodHistory> {
    let mut players: BTreeMap<u128, FoodHistory> = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((uuid, item)) = line.split_once(['\t', ' ']) else {
            continue;
        };
        let Some(uuid) = parse_uuid_hex(uuid) else {
            continue;
        };
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        players
            .entry(uuid)
            .or_default()
            .foods
            .insert(item.to_owned());
    }
    players
}

pub fn format_uuid_hex(uuid: u128) -> String {
    format!("{uuid:032x}")
}

pub fn parse_uuid_hex(text: &str) -> Option<u128> {
    let text = text.trim();
    (text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u128::from_str_radix(text, 16).ok())
        .flatten()
}

/// What `/foodlist` asks for (`FoodListCommand.java:49-55`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Size(Option<String>),
    Sync(Option<String>),
    Clear(Option<String>),
    Unknown,
}

pub fn parse_command(line: &str) -> Command {
    let mut words = line.split_whitespace();
    if words.next() != Some("foodlist") {
        return Command::Unknown;
    }
    let Some(sub) = words.next() else {
        return Command::Unknown;
    };
    let target = words.next().map(str::to_owned);
    if words.next().is_some() {
        return Command::Unknown;
    }
    match sub {
        "size" => Command::Size(target),
        "sync" => Command::Sync(target),
        "clear" => Command::Clear(target),
        _ => Command::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn defaults_match_upstream() {
        let c = config();
        assert_eq!(c.base_hearts, 10);
        assert_eq!(c.hearts_per_milestone, 2);
        assert_eq!(c.milestones, vec![5, 10, 15, 20, 25]);
        assert_eq!(c.minimum_food_value, 1);
        assert!(c.blacklist.is_empty());
        assert!(c.whitelist.is_empty());
        assert!(!c.reset_on_death);
        assert!(!c.limit_progression_to_survival);
    }

    #[test]
    fn config_overrides_and_comments() {
        let c = parse_config(
            "# a comment\n\
             base_hearts = 6\n\
             hearts_per_milestone=1\n\
             milestones = 1, 2 ,3\n\
             blacklist = minecraft:rotten_flesh, *:candy\n\
             whitelist =\n\
             minimum_food_value = 2\n\
             reset_on_death = true\n\
             limit_progression_to_survival = yes\n\
             unknown_key = 9\n",
        );
        assert_eq!(c.base_hearts, 6);
        assert_eq!(c.hearts_per_milestone, 1);
        assert_eq!(c.milestones, vec![1, 2, 3]);
        assert_eq!(
            c.blacklist,
            vec!["minecraft:rotten_flesh".to_owned(), "*:candy".to_owned()]
        );
        assert!(c.whitelist.is_empty());
        assert_eq!(c.minimum_food_value, 2);
        assert!(c.reset_on_death);
        assert!(c.limit_progression_to_survival);
    }

    #[test]
    fn config_bad_values_keep_defaults_and_clamp() {
        let c = parse_config(
            "base_hearts = nope\nhearts_per_milestone = -4\nmilestones = 4,x\nminimum_food_value = 5000\nreset_on_death = maybe\n",
        );
        assert_eq!(c.base_hearts, 10);
        assert_eq!(c.hearts_per_milestone, 0); // clamped to 0
        assert_eq!(c.milestones, vec![4]);
        assert_eq!(c.minimum_food_value, 1000);
        assert!(!c.reset_on_death);
    }

    #[test]
    fn config_round_trips_through_the_written_defaults() {
        let c = Config::default();
        assert_eq!(parse_config(&format_config(&c)), c);
        let c = parse_config(
            "base_hearts=6\nhearts_per_milestone=1\nmilestones=1,2\nblacklist=minecraft:rotten_flesh\nreset_on_death=true\n",
        );
        assert_eq!(parse_config(&format_config(&c)), c);
    }

    #[test]
    fn empty_milestones_are_no_bonus_not_a_panic() {
        let c = parse_config("milestones =\n");
        assert!(c.milestones.is_empty());
        let p = progress(3, &c);
        assert!(p.has_reached_max);
        assert_eq!(p.next_milestone, None);
        assert_eq!(health_modifier_amount(3, &c), 0);
    }

    #[test]
    fn glob_matches_whole_ids() {
        assert!(glob_matches("minecraft:apple", "minecraft:apple"));
        assert!(!glob_matches("minecraft:apple", "minecraft:apples"));
        assert!(glob_matches("minecraft:*", "minecraft:apple"));
        assert!(glob_matches("*:apple", "minecraft:apple"));
        assert!(glob_matches("*", "anything:at_all"));
        assert!(glob_matches("minecraft:*:pie", "minecraft:pumpkin:pie"));
        assert!(!glob_matches("minecraft:*:pie", "minecraft:pumpkin:cake"));
        assert!(glob_matches("a*b*c", "axxbyyc"));
        assert!(!glob_matches("a*b*c", "axxbyy"));
    }

    #[test]
    fn nutrition_and_filters_decide_counting() {
        let c = config();
        assert!(should_count("minecraft:apple", 4, &c));
        assert!(!should_count("minecraft:apple", 0, &c));
        let mut black = c.clone();
        black.blacklist = vec!["minecraft:*".to_owned()];
        assert!(!should_count("minecraft:apple", 4, &black));
        let mut white = black.clone();
        white.whitelist = vec!["minecraft:apple".to_owned()];
        assert!(should_count("minecraft:apple", 4, &white));
        assert!(!should_count("minecraft:bread", 5, &white));
    }

    #[test]
    fn recording_remembers_filtered_food_but_does_not_count_it() {
        let c = config();
        let mut h = FoodHistory::new();
        let first = h.record("minecraft:apple", 4, &c);
        assert_eq!(
            first,
            Added {
                newly_added: true,
                counts: true
            }
        );
        let again = h.record("minecraft:apple", 4, &c);
        assert_eq!(
            again,
            Added {
                newly_added: false,
                counts: true
            }
        );
        let thin = h.record("minecraft:potion", 0, &c);
        assert_eq!(
            thin,
            Added {
                newly_added: true,
                counts: false
            }
        );
        assert!(h.contains("minecraft:potion"));
        assert_eq!(h.len(), 2);
        assert_eq!(
            h.counted(|id| (id == "minecraft:apple").then_some(4), &c),
            1
        );
    }

    #[test]
    fn milestones_match_upstream_thresholds() {
        let c = config();
        for (eaten, achieved) in [
            (0, 0),
            (4, 0),
            (5, 1),
            (9, 1),
            (10, 2),
            (24, 4),
            (25, 5),
            (100, 5),
        ] {
            assert_eq!(milestones_achieved(eaten, &c), achieved, "eaten {eaten}");
        }
    }

    #[test]
    fn health_is_four_points_per_default_milestone() {
        let c = config();
        assert_eq!(health_modifier_amount(0, &c), 0);
        assert_eq!(health_modifier_amount(4, &c), 0);
        assert_eq!(health_modifier_amount(5, &c), 4);
        assert_eq!(health_modifier_amount(10, &c), 8);
        assert_eq!(health_modifier_amount(25, &c), 20);
    }

    #[test]
    fn base_hearts_and_hearts_per_milestone_scale_the_bonus() {
        let c = parse_config("base_hearts=6\nhearts_per_milestone=1\nmilestones=1,2\n");
        assert_eq!(health_modifier_amount(0, &c), -8);
        assert_eq!(health_modifier_amount(1, &c), -6);
        assert_eq!(health_modifier_amount(2, &c), -4);
    }

    #[test]
    fn progress_reports_next_milestone_and_cap() {
        let c = config();
        let p = progress(3, &c);
        assert_eq!(p.foods_eaten, 3);
        assert_eq!(p.milestones_achieved, 0);
        assert!(!p.has_reached_max);
        assert_eq!(p.next_milestone, Some(5));
        assert_eq!(p.foods_until_next, Some(2));
        let p = progress(7, &c);
        assert_eq!(p.next_milestone, Some(10));
        assert_eq!(p.foods_until_next, Some(3));
        let p = progress(25, &c);
        assert!(p.has_reached_max);
        assert_eq!(p.next_milestone, None);
        assert_eq!(p.foods_until_next, None);
    }

    #[test]
    fn storage_round_trips_histories_and_rejects_junk() {
        let mut players = BTreeMap::new();
        let mut a = FoodHistory::new();
        a.record("minecraft:apple", 4, &config());
        a.record("minecraft:bread", 5, &config());
        let mut b = FoodHistory::new();
        b.record("minecraft:carrot", 3, &config());
        players.insert(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef, a);
        players.insert(1, b);
        let text = format_store(&players);
        assert!(text.starts_with(STORE_HEADER));
        let parsed = parse_store(&text);
        assert_eq!(parsed, players);
        let junk = format!(
            "{STORE_HEADER}\nnot-a-uuid\tminecraft:apple\n\
             0123456789abcdef0123456789abcdef\n\
             0123456789abcdef0123456789abcdef\t\n\
             0123456789abcdef0123456789abcdef\tminecraft:apple\n"
        );
        let parsed = parse_store(&junk);
        assert_eq!(parsed.len(), 1);
        assert!(parsed[&0x0123_4567_89ab_cdef_0123_4567_89ab_cdef].contains("minecraft:apple"));
    }

    #[test]
    fn uuids_are_thirty_two_hex_digits() {
        let uuid = 0xdead_beef_0000_0000_1234_5678_9abc_def0;
        let text = format_uuid_hex(uuid);
        assert_eq!(text.len(), 32);
        assert_eq!(parse_uuid_hex(&text), Some(uuid));
        assert_eq!(parse_uuid_hex("dead-beef"), None);
        assert_eq!(parse_uuid_hex(&"z".repeat(32)), None);
    }

    #[test]
    fn command_parsing_covers_the_three_subcommands() {
        assert_eq!(parse_command("foodlist size"), Command::Size(None));
        assert_eq!(
            parse_command("foodlist size Alex"),
            Command::Size(Some("Alex".to_owned()))
        );
        assert_eq!(parse_command("foodlist sync"), Command::Sync(None));
        assert_eq!(
            parse_command("foodlist clear  Steve "),
            Command::Clear(Some("Steve".to_owned()))
        );
        for line in [
            "foodlist",
            "foodlist nope",
            "foodlist size a b",
            "food list size",
            "",
        ] {
            assert_eq!(parse_command(line), Command::Unknown, "{line}");
        }
    }

    #[test]
    fn clear_empty_history_is_a_no_op() {
        let mut h = FoodHistory::new();
        h.record("minecraft:apple", 4, &config());
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.counted(|_| Some(4), &config()), 0);
        assert_eq!(
            health_modifier_amount(h.counted(|_| Some(4), &config()), &config()),
            0
        );
    }

    #[test]
    fn missing_items_do_not_count() {
        let c = config();
        let mut h = FoodHistory::new();
        h.record("gone:stew", 6, &c);
        h.record("minecraft:apple", 4, &c);
        assert_eq!(
            h.counted(|id| (id == "minecraft:apple").then_some(4), &c),
            1
        );
    }
}
