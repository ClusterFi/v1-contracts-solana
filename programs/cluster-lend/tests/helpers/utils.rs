use solana_sdk::signature::Keypair;

pub fn ui_to_native(ui_amount: f64, decimals: u8) -> u64 {
    (ui_amount * (10u64.pow(decimals as u32) as f64)) as u64
}

pub fn native_to_ui(native_amount: u64, decimals: u8) -> f64 {
    native_amount as f64 / 10u64.pow(decimals as u32) as f64
}

pub fn clone_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_bytes(&keypair.to_bytes()).unwrap()
}
