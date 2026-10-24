fn roll(state) {
    let value = state.random(1, 6);
    let rolls = state.get("rolls");
    rolls.push(value);
    state.set("rolls", rolls);
    state.set("last", value);
}
