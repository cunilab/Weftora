fn refresh_schedule(state) {
    let hour = state.get("hour");
    let open = state.get("location") == "Market" && hour >= 9 && hour < 12 && !state.get("worked");
    let schedule = state.get("schedule");
    schedule.open = open;
    state.set("schedule", schedule);
    state.set("shift_open", open);
}
fn work_shift(state) {
    let items = state.get("inventory");
    if state.get("location") == "Market" && state.get("shift_open") && !state.get("worked") && state.get("energy") >= 2 && items.contains("permit") {
        let base_pay = 3;
        let pay = base_pay + state.random(0, 2);
        state.set("coins", state.get("coins") + pay);
        state.set("earned", pay);
        state.set("energy", state.get("energy") - 2);
        state.set("worked", true);
        items.push("receipt");
        state.set("inventory", items);
        state.set("hour", state.get("hour") + 2);
        state.set("status", "Shift complete.");
    } else {
        state.set("status", "Shift unavailable.");
    }
}
fn buy_snack(state) {
    let items = state.get("inventory");
    if state.get("location") == "Market" && state.get("coins") >= 2 && !items.contains("snack") {
        state.set("coins", state.get("coins") - 2);
        items.push("snack");
        state.set("inventory", items);
        state.set("hour", state.get("hour") + 1);
        state.set("status", "Snack bought.");
    }
}
fn eat_snack(state) {
    let items = state.get("inventory");
    if state.get("location") == "Home" && items.contains("snack") {
        let remaining = [];
        for item in items { if item != "snack" { remaining.push(item); } }
        state.set("inventory", remaining);
        let energy = state.get("energy") + 3;
        if energy > 6 { energy = 6; }
        state.set("energy", energy);
        state.set("hour", state.get("hour") + 1);
        state.set("status", "A good meal.");
    }
}
fn rest(state) {
    let energy = state.get("energy") + 2;
    if energy > 6 { energy = 6; }
    state.set("energy", energy);
    state.set("hour", state.get("hour") + 1);
    state.set("status", "Rested at home.");
}
