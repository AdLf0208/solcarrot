// End of a player's server tick (API 31): detect a finished food use and keep
// the max-health modifier in step with the milestone progress.
fn observe_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::solcarrot_state::on_player_tick(game);
}
