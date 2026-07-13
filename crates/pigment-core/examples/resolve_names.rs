use pigment_core::roblox_api;
fn main() {
    for uid in [6035872082u64, 3808081382] {
        println!("universe {uid} -> {:?}", roblox_api::game_name(uid));
    }
}
