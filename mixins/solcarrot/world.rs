// A world opened or closed (API 44): load/save `<world>/solcarrot/foods.tsv`.
fn observe_world(game: &mut crate::game::Game<'_>, phase: WorldPhase) {
    crate::game::solcarrot_state::on_world_lifecycle(game, phase);
}
