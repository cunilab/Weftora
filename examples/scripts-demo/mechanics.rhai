fn pack(state, amount) {
    let items = state.get("items");
    let offers = ["key", "ticket", "pass"];
    let limit = amount;
    let i = 0;
    while i < limit {
        let item = offers[i];
        if !items.contains(item) { items.push(item); }
        i += 1;
    }
    state.set("items", items);
    let schedule = state.get("schedule");
    schedule.energy = 10 - items.len();
    schedule.morning = "station";
    state.set("schedule", schedule);
    state.set("item_count", items.len());
    state.set("status", "Kit packed");
    state.emit("kit.packed", true);
}
