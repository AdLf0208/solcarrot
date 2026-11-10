// Before the server starts (API 44): load or create `config/solcarrot.conf`.
fn observe_about_to_start(game: &mut crate::game::Game<'_>) {
    crate::game::solcarrot_state::on_server_about_to_start(game);
}
