pub fn short_fingerprint(public_key: &pkarr::PublicKey) -> String {
    let z32 = public_key.to_z32();
    z32[..8].to_string()
}
