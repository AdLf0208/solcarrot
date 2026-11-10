// A player died (API 21): clear the history when `reset_on_death` is on.
fn observe_player_died(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::solcarrot_state::on_player_died(game, player);
}
